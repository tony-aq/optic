import { Command, flags } from '@oclif/command';
import { ensureDaemonStarted } from '@useoptic/cli-server';
import path from 'path';
import colors from 'colors';
import {
  IApiCliConfig,
  IOpticTaskRunnerConfig,
  IPathMapping,
  TargetPortUnavailableError,
  deprecationLogger,
  isTestTask,
} from '@useoptic/cli-config';
import { opticTaskToProps, trackUserEvent } from './analytics';
import { lockFilePath } from './paths';
import { Client, SpecServiceClient } from '@useoptic/cli-client';
import findProcess from 'find-process';
import stripAnsi from 'strip-ansi';
import {
  ExitedTaskWithLocalCli,
  StartedTaskWithLocalCli,
} from '@useoptic/analytics';
import {
  cleanupAndExit,
  CommandAndProxySessionManager,
  developerDebugLogger,
  fromOptic,
  IOpticTaskRunner,
  loadPathsAndConfig,
  makeUiBaseUrl,
  warningFromOptic,
} from '@useoptic/cli-shared';
import * as uuid from 'uuid';
import { CliTaskSession } from '@useoptic/cli-shared/build/tasks';
import { CaptureSaverWithDiffs } from '@useoptic/cli-shared/build/captures/avro/file-system/capture-saver-with-diffs';
import { EventEmitter } from 'events';
import { Config } from '../config';
import { computeCoverage, printCoverage } from './coverage';
import { spawnProcessReturnExitCode } from './spawn-process';
import { getCaptureId } from './git/git-context-capture';
import { getSpecEventsFrom } from '@useoptic/cli-config/build/helpers/read-specification-json';
import { linkToCapture, linkToDiffs } from './ui-links';
import { RunTaskVerboseLogger } from './verbose/verbose';
import { IHttpInteraction } from '@useoptic/optic-domain';
import { LocalCliSpectacle } from '@useoptic/spectacle-shared';
import * as opticEngine from '@useoptic/optic-engine-wasm';

export const runCommandFlags = {
  'print-coverage': flags.boolean({
    char: 'c',
    default: false,
    required: false,
  }),
  'collect-diffs': flags.boolean({
    char: 'd',
    default: true,
    required: false,
  }),
  'exit-on-diff': flags.boolean({
    default: false,
    required: false,
    description:
      'When a capture session ends, if a diff is detected Optic will exit with exit code 1. This takes priority over pass-exit-code.',
  }),
  'transparent-proxy': flags.boolean({
    default: false,
    required: false,
  }),
  'pass-exit-code': flags.boolean({
    default: false,
    required: false,
    description:
      'Passes through the exit code from your task (or dependent task). exit-on-diff overrides this when a diff is detected.',
  }),
  ci: flags.boolean({
    default: false,
    required: false,
    description: 'Enables CI-specific behavior',
  }),
  verbose: flags.boolean({
    default: false,
    required: false,
    description: 'Verbose logging of Optic lifecycle and sample collection',
  }),
};
export interface LocalCliTaskFlags {
  'print-coverage'?: boolean;
  'collect-diffs'?: boolean;
  'exit-on-diff'?: boolean;
  'transparent-proxy'?: boolean;
  'pass-exit-code'?: boolean;
  ci?: boolean;
  verbose?: boolean;
}

export async function LocalTaskSessionWrapper(
  cli: Command,
  taskName: string,
  flags: LocalCliTaskFlags
) {
  // hijack the config deprecation log to format nicely for the CLI
  deprecationLogger.log = (msg: string) => {
    cli.log(
      warningFromOptic(
        'optic.yml deprecation: ' +
          stripAnsi(msg).replace(deprecationLogger.namespace, '').trim()
      )
    );
  };
  deprecationLogger.enabled = true;

  if (flags['ci']) {
    flags['print-coverage'] = true;
    flags['pass-exit-code'] = true;
    flags['collect-diffs'] = true;
    flags['exit-on-diff'] = true;
  }

  const usesTaskSpecificBoundary = flags['ci'] || flags['exit-on-diff'];

  const { paths, config } = await loadPathsAndConfig(cli);

  await getSpecEventsFrom(paths.specStorePath);

  const captureId = usesTaskSpecificBoundary
    ? uuid.v4()
    : await getCaptureId(paths);

  const logger = new RunTaskVerboseLogger(
    flags.verbose || false,
    taskName,
    flags,
    paths.basePath
  );

  const runner = new LocalCliTaskRunner(captureId, paths, taskName, logger, {
    shouldCollectDiffs: flags['collect-diffs'] !== false,
    shouldExitOnDiff: flags['exit-on-diff'] !== false,
    shouldTransparentProxy: flags['transparent-proxy'] !== false,
    shouldPassThroughExitCode: flags['pass-exit-code'] !== false,
    isRunningInCI: flags['ci'] !== false,
  });
  const session = new CliTaskSession(runner);
  const task = config.tasks[taskName];

  if (!task) {
    cli.log(
      fromOptic(
        `Task ${colors.grey.bold(
          taskName
        )} does not exist. Try one of these ${colors.grey.bold(
          'api run <taskname>'
        )}`
      )
    );
    return cli.log(
      Object.keys(config.tasks || [])
        .map((i) => '- ' + i)
        .sort()
        .join('\n')
    );
  }

  if (task && isTestTask(task)) {
    cli.log(
      fromOptic(`Running dependent task ${colors.grey.bold(task.useTask!)}...`)
    );
    await session.start(cli, config, task.useTask!, task.command!);
  } else {
    await session.start(cli, config, taskName);
  }

  if (flags['print-coverage']) {
    const diff_maps = await computeCoverage(paths, captureId);
    await printCoverage(paths, diff_maps.with_diffs, diff_maps.without_diffs);
  }

  if (runner.foundDiff && flags['exit-on-diff']) {
    return await cleanupAndExit(1);
  }

  return await cleanupAndExit(
    flags['pass-exit-code'] !== false ? runner.exitWithCode : 0
  );
}

export class LocalCliTaskRunner implements IOpticTaskRunner {
  public foundDiff: boolean = false;
  public exitWithCode: number | undefined;

  constructor(
    private captureId: string,
    private paths: IPathMapping,
    private taskName: string,
    private logger: RunTaskVerboseLogger,
    private options: {
      shouldCollectDiffs: boolean;
      shouldExitOnDiff: boolean;
      shouldTransparentProxy: boolean;
      shouldPassThroughExitCode: boolean;
      isRunningInCI: boolean;
    }
  ) {}

  async run(
    cli: Command,
    config: IApiCliConfig,
    taskConfig: IOpticTaskRunnerConfig,
    commandToRunWhenStarted?: string
  ): Promise<void> {
    ////////////////////////////////////////////////////////////////////////////////

    this.logger.logConfigMeaning(taskConfig);

    ////////////////////////////////////////////////////////////////////////////////

    const blockers = await findProcess('port', taskConfig.proxyConfig.port);
    if (blockers.length > 0) {
      const conflict = `${blockers
        .map((x) => `[pid ${x.pid}]: ${x.cmd}`)
        .join('\n')}`;
      this.logger.portTaken(taskConfig.proxyConfig.port);
      throw new TargetPortUnavailableError(
        `Optic could not start its proxy server on port ${taskConfig.proxyConfig.port} \n ${conflict}`
      );
    }

    ////////////////////////////////////////////////////////////////////////////////

    const daemonState = await ensureDaemonStarted(
      lockFilePath,
      Config.apiBaseUrl
    );
    const apiBaseUrl = `http://localhost:${daemonState.port}/api`;
    developerDebugLogger(`api base url: ${apiBaseUrl}`);
    const cliClient = new Client(apiBaseUrl);

    ////////////////////////////////////////////////////////////////////////////////
    developerDebugLogger('finding matching daemon session');

    const { cwd } = this.paths;
    const cliSession = await cliClient.findSession(
      cwd,
      taskConfig,
      this.captureId
    );
    developerDebugLogger({ cliSession });

    const spectacleBaseUrl = `${apiBaseUrl}/specs/${cliSession.session.id}`;

    ////////////////////////////////////////////////////////////////////////////////
    const spectacle = new LocalCliSpectacle(spectacleBaseUrl, opticEngine);
    const idQuery = await spectacle.query<any>({
      query: `{
        metadata {
          id
        }
      }`,
      variables: {},
    });
    const specId = idQuery?.data?.metadata?.id ?? 'anon-spec-id';

    // See here for why this is split into separate queries: https://github.com/opticdev/optic/pull/1083#issuecomment-893522118
    // TL;DR querying for specId will create if it doesn't exist (which adds a batch commit), and then reload spectacle
    // but that could race with the `batchCommits` query, so better to serialize the queries to make sure things are loaded
    const batchCommitQuery = await spectacle.query<any>({
      query: `{
        batchCommits {
          createdAt
        }
      }`,
      variables: {},
    });

    const createdAt = batchCommitQuery?.data?.batchCommits
      ?.map((commit: any) => new Date(commit?.createdAt))
      ?.sort((a: Date, b: Date) => a.getTime() - b.getTime())?.[0];

    await trackUserEvent({
      apiName: config.name,
      specId,
      event: StartedTaskWithLocalCli({
        inputs: opticTaskToProps(this.taskName, taskConfig),
        createdAt,
        cwd: this.paths.cwd,
        captureId: this.captureId,
      }),
    });

    ////////////////////////////////////////////////////////////////////////////////

    const uiBaseUrl = makeUiBaseUrl(daemonState);

    if (!this.options.isRunningInCI) {
      const uiUrl = linkToDiffs(uiBaseUrl, cliSession.session.id);
      cli.log(fromOptic(`Review the API Diff at ${uiUrl}`));
    }

    ////////////////////////////////////////////////////////////////////////////////
    const { capturesPath } = this.paths;
    const captureId = this.captureId;
    const eventEmitter = new EventEmitter();
    const specServiceClient = new SpecServiceClient(
      cliSession.session.id,
      eventEmitter,
      apiBaseUrl
    );
    const persistenceManager = new CaptureSaverWithDiffs(
      {
        captureBaseDirectory: capturesPath,
        captureId,
        shouldCollectDiffs: this.options.shouldCollectDiffs,
      },
      config,
      specServiceClient
    );

    ////////////////////////////////////////////////////////////////////////////////
    process.env.OPTIC_ENABLE_CAPTURE_BODY = 'yes';
    process.env.OPTIC_ENABLE_TRANSPARENT_PROXY = this.options
      .shouldTransparentProxy
      ? 'yes'
      : process.env.OPTIC_ENABLE_TRANSPARENT_PROXY;

    const testCommand = commandToRunWhenStarted
      ? async () => {
          console.log(
            fromOptic(
              'Running test command ' +
                colors.grey.bold(commandToRunWhenStarted)
            )
          );

          const exitCodeOfTestProcess = await spawnProcessReturnExitCode(
            commandToRunWhenStarted!,
            {
              OPTIC_PROXY_PORT: taskConfig.proxyConfig.port.toString(),
              OPTIC_PROXY_HOST: taskConfig.proxyConfig.host.toString(),
              OPTIC_PROXY: `http://${taskConfig.proxyConfig.host.toString()}:${taskConfig.proxyConfig.port.toString()}`,
            }
          );

          if (this.options.shouldPassThroughExitCode) {
            this.exitWithCode = exitCodeOfTestProcess;
          }
        }
      : undefined;

    const sessionManager = new CommandAndProxySessionManager(
      taskConfig,
      testCommand
    );

    sessionManager.inboundProxy.events.on(
      'sample',
      (sample: IHttpInteraction) => {
        this.logger.sample(sample);
      }
    );

    await sessionManager.run(persistenceManager);

    if (!commandToRunWhenStarted && this.options.shouldPassThroughExitCode) {
      this.exitWithCode = sessionManager.getExitCodeOfProcess();
    }

    if (this.exitWithCode !== undefined)
      this.logger.commandExitCode(this.exitWithCode);

    ////////////////////////////////////////////////////////////////////////////////
    await cliClient.markCaptureAsCompleted(cliSession.session.id, captureId);
    const summary = await specServiceClient.getCaptureStatus(captureId);
    const sampleCount = summary.interactionsCount;
    const hasDiff = summary.diffsCount > 0;

    await trackUserEvent({
      apiName: config.name,
      specId,
      event: ExitedTaskWithLocalCli({
        interactionCount: sampleCount,
        inputs: opticTaskToProps('', taskConfig),
        captureId: this.captureId,
      }),
    });

    if (hasDiff) {
      const uiUrl = linkToCapture(uiBaseUrl, cliSession.session.id, captureId);

      const usesTaskSpecificCapture = this.options.shouldExitOnDiff;

      if (this.options.isRunningInCI) {
        cli.log(`Observed Unexpected API Behavior.`);
        // print output for `api status` here
      } else if (usesTaskSpecificCapture) {
        cli.log(
          fromOptic(`Observed Unexpected API Behavior. Review at ${uiUrl}`)
        );
      } else {
        cli.log(
          fromOptic(`Observed Unexpected API Behavior. Run "api status"`)
        );
      }

      this.foundDiff = true;
    } else {
      if (sampleCount > 0) {
        cli.log(
          fromOptic(`No API Diff Observed for ${sampleCount} interactions`)
        );
      }
    }

    this.logger.results(
      sampleCount,
      this.foundDiff,
      commandToRunWhenStarted
        ? false
        : typeof this.exitWithCode === 'undefined',
      this.exitWithCode
    );
  }
}
