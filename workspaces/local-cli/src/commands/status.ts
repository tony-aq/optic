import { Command, flags } from '@oclif/command';
//@ts-ignore
import gitRev from '../shared/git/git-rev-sync-insourced.js';
import { ensureDaemonStarted } from '@useoptic/cli-server';
import { lockFilePath } from '../shared/paths';
import { Config } from '../config';
//@ts-ignore
import groupBy from 'lodash.groupby';
//@ts-ignore
import padLeft from 'pad-left';
import {
  cleanupAndExit,
  developerDebugLogger,
  fromOptic,
  makeUiBaseUrl,
  userDebugLogger,
} from '@useoptic/cli-shared';
import { Client } from '@useoptic/cli-client';
import {
  getPathsRelativeToConfig,
  IApiCliConfig,
  IPathMapping,
  readApiConfig,
} from '@useoptic/cli-config';
import { isInRepo } from '../shared/git/git-context-capture';
import colors from 'colors';
import { getUser } from '../shared/analytics';
import { IDiff } from '@useoptic/cli-shared/build/diffs/diffs';
import { IInteractionTrail } from '@useoptic/cli-shared/build/diffs/interaction-trail';
import { IRequestSpecTrail } from '@useoptic/cli-shared/build/diffs/request-spec-trail';
import {
  IRequestBodyForTrailParser,
  IResponseBodyForTrailParser,
} from '@useoptic/cli-shared/src/diffs/trail-parsers';
import sortBy from 'lodash.sortby';
import { KnownEndpoint } from '../shared/coverage';
import openBrowser from 'react-dev-utils/openBrowser';
import { linkToCapture } from '../shared/ui-links';
import {
  LocalCliCapturesService,
  LocalCliConfigRepository,
  LocalCliSpectacle,
} from '@useoptic/spectacle-shared';
import * as opticEngine from '@useoptic/diff-engine-wasm/engine/build';
import { response } from 'express';

export default class Status extends Command {
  static description = 'lists API diffs observed since your last git commit';

  static flags = {
    'pre-commit': flags.boolean(),
    review: flags.boolean(),
  };

  async run() {
    const { flags } = this.parse(Status);

    const timeStated = Date.now();

    let diffFound = false;
    const exitOnDiff = Boolean(flags['pre-commit']);
    const openReviewPage = Boolean(flags['review']);

    let { paths, config } = (await this.requiresSpec())!;

    await this.requiresInGit(paths.basePath);

    const captureId = 'ccc'; ///await getCaptureId(paths);
    developerDebugLogger('using capture id ', captureId);

    if (openReviewPage) {
      return this.openDiffPage(paths.cwd, captureId);
    }

    /// ^ setting everything up.
    // captureId

    const daemonState = await ensureDaemonStarted(
      lockFilePath,
      Config.apiBaseUrl
    );

    const apiBaseUrl = `http://localhost:${daemonState.port}/api`;
    developerDebugLogger(`api base url: ${apiBaseUrl}`);
    const cliClient = new Client(apiBaseUrl);

    const cliSession = await cliClient.findSession(paths.cwd, null, null);

    const sessionApiBaseUrl = `http://localhost:${daemonState.port}/api/specs/${cliSession.session.id}`;
    console.log(sessionApiBaseUrl);
    const spectacle = new LocalCliSpectacle(sessionApiBaseUrl, opticEngine);
    const capturesService = new LocalCliCapturesService({
      baseUrl: sessionApiBaseUrl,
      spectacle,
    });

    const configRepository = new LocalCliConfigRepository({
      baseUrl: sessionApiBaseUrl,
      spectacle,
    });

    const startDiffResult = await capturesService.startDiff(
      'status1', // some random ID
      captureId
    );

    // start a loading state
    const diffService = await startDiffResult.onComplete;
    //end our loading state

    const { diffs } = await diffService.listDiffs();
    const urls = await diffService.listUnrecognizedUrls();

    const requests: any = await spectacle.query({
      query: `{
        requests {
          id
          pathId
          method
          absolutePathPatternWithParameterNames
          bodies {
            contentType
            rootShapeId
          }
          responses {
            id
            statusCode
            bodies {
              contentType
              rootShapeId
            }
          }
        }
      }`,
      variables: {},
    });

    const endpoints = new Map();
    const requestBodies: IRequestBodyForTrailParser[] = [];
    const responseBodies: IResponseBodyForTrailParser[] = [];

    for (const request of requests.data.requests) {
      const endpoint = new Map();
      endpoint.set(
        request.method,
        request.absolutePathPatternWithParameterNames
      );
      endpoints.set(request.pathId, endpoint);

      for (const requestBody of request.bodies) {
        requestBodies.push({
          requestId: request.id,
          pathId: request.pathId,
          method: request.method,
          contentType: requestBody.contentType,
          rootShapeId: requestBody.rootShapeId,
        });
      }

      for (const response of request.responses) {
        for (const responseBody of response.bodies) {
          responseBodies.push({
            responseId: response.id,
            pathId: request.pathId,
            method: request.method,
            contentType: responseBody.contentType,
            rootShapeId: responseBody.rootShapeId,
            statusCode: response.statusCode,
          });
        }
      }
    }

    // To get the absolute path pattern, use something like:
    // console.log(endpoints.get('path_giU9lpUDNH').get('GET'));

    // console.log(requestBodies);
    // console.log(responseBodies);

    // list all the endpoints from Spectacle ^
    console.log(diffs); // {pathId, method, diff}[]
    //group by pathId, method -- get the actual absolutePath from Spectacle by looking up pathId and method
    //print out the name of all endpoints with diffs
    console.log(urls);

    this.printStatus([], {}, [{ method: 'GET', path: '/abc', count: 1 }]);
    // trail-parsers and move to cli-shared

    // const diffsPromise = this.getDiffsAndEvents(paths, captureId);
    // diffsPromise.catch((e) => {
    //   console.error(e);
    //   this.printStatus([], [], []);
    // });
    // diffsPromise.then(async ({ diffs, undocumentedUrls, events }) => {
    //   const rfcBaseState = makeDiffRfcBaseStateFromEvents(events);
    //   const diffsRaw: IDiff[] = diffs.map((i: any) => i[0]);
    //
    //   const locations = diffsRaw
    //     .map((i) => {
    //       return locationForTrails(
    //         extractRequestsTrail(i),
    //         extractInteractionTrail(i),
    //         rfcBaseState
    //       )!;
    //     })
    //     .filter(Boolean);
    //
    //   const diffsGroupedByPathAndMethod = groupBy(
    //     locations,
    //     (i: any) => `${i.method}.${i.pathId}`
    //   );
    //
    //   const endpointsWithDiffs = getSpecEndpoints(
    //     rfcBaseState.queries
    //   ).filter((i) =>
    //     locations.find(
    //       (withDiff) =>
    //         withDiff.pathId === i.pathId && withDiff.method === i.method
    //     )
    //   );
    //
    //   this.printStatus(
    //     endpointsWithDiffs,
    //     diffsGroupedByPathAndMethod,
    //     undocumentedUrls
    //   );
    //
    //   diffFound = diffs.length > 0 || undocumentedUrls.length > 0;
    //
    //   await trackUserEvent(
    //     config.name,
    //     StatusRun.withProps({
    //       captureId,
    //       diffCount: diffs.length,
    //       undocumentedCount: undocumentedUrls.length,
    //       timeMs: Date.now() - timeStated,
    //     })
    //   );
    // });
    //
    // diffsPromise.finally(() => {
    //   if (diffFound && exitOnDiff) {
    //     console.error(
    //       colors.red('Optic detected an API diff. Run "api status --review"')
    //     );
    //     process.exit(1);
    //   }
    //   cleanupAndExit();
    // });
  }

  async exitWithError(error: string) {
    this.log(fromOptic(error));
    process.exit(0);
  }

  async requiresInGit(basepath: string) {
    if (isInRepo(basepath)) {
      return;
    } else {
      await this.exitWithError(
        `"${colors.bold('api status')}" only works when Optic is in a Git repo`
      );
    }
  }

  async requiresSpec(): Promise<
    | {
        paths: IPathMapping;
        config: IApiCliConfig;
      }
    | undefined
  > {
    let paths: IPathMapping;
    let config: IApiCliConfig;

    try {
      paths = await getPathsRelativeToConfig();
      config = await readApiConfig(paths.configPath);
      return { paths, config };
    } catch (e) {
      userDebugLogger(e);
      await this.exitWithError(
        `No optic.yml file found here. Add Optic to your API by running ${colors.bold(
          'api init'
        )}`
      );
    }
  }

  private printStatus(
    endpointsWithDiffs: KnownEndpoint[],
    diffsGroupedByPathAndMethod: { [key: string]: any[] },
    undocumentedUrls: { path: string; method: string; count: number }[]
  ) {
    const diffCount = (i: KnownEndpoint) =>
      (diffsGroupedByPathAndMethod[`${i.method}.${i.pathId}`] || []).length;

    const sorted = sortBy(endpointsWithDiffs, (i) => -diffCount(i));
    const changed = sorted.map((i) =>
      colors.yellow(generateEndpointString(i.method, i.fullPath))
    );

    if (changed.length === 0) {
      this.log(`✓  No diffs observed for existing endpoints`);
    } else {
      this.log(colors.bold(`   Diffs observed for existing endpoints`));
      this.log(
        colors.grey(
          `     (use ${colors.bold(
            '"api status --review"'
          )} to review in the UI`
        )
      );
      this.log(changed.join('\n'));
    }

    const ordered = sortBy(undocumentedUrls, ['count', 'path']).reverse();

    const onlyShow = 15;
    const newUrls = ordered
      .slice(0, onlyShow)
      .map((i) => colors.green(generateEndpointString(i.method, i.path)));

    if (ordered.length === 0) {
      this.log(`✓  No undocumented URLs observed`);
    } else {
      this.log(colors.bold(`   Undocumented URLs observed`));
      this.log(
        colors.grey(
          `      (use ${colors.bold(
            '"api status --review"'
          )} to start documenting them`
        )
      );
      this.log(newUrls.join('\n'));
      if (ordered.length > onlyShow) {
        this.log(`   and ${ordered.length - onlyShow} more...`);
      }
    }
  }

  async openDiffPage(basePath: string, captureId: string) {
    const daemonState = await ensureDaemonStarted(
      lockFilePath,
      Config.apiBaseUrl
    );

    const apiBaseUrl = `http://localhost:${daemonState.port}/api`;
    developerDebugLogger(`api base url: ${apiBaseUrl}`);
    const cliClient = new Client(apiBaseUrl);
    cliClient.setIdentity(await getUser());
    const cliSession = await cliClient.findSession(basePath, null, null);
    developerDebugLogger({ cliSession });
    const uiBaseUrl = makeUiBaseUrl(daemonState);
    const uiUrl = `${uiBaseUrl}/apis/${cliSession.session.id}/review/${captureId}`;
    openBrowser(linkToCapture(uiBaseUrl, cliSession.session.id, captureId));
    cleanupAndExit();
  }
}

function generateEndpointString(method: string, fullPath: string) {
  return `${colors.bold(padLeft(method, 13, ' '))}   ${fullPath}`;
}

function extractInteractionTrail(i: IDiff): IInteractionTrail {
  const kind: string = Object.keys(i)[0];
  // @ts-ignore
  return i[kind]!.interactionTrail;
}
function extractRequestsTrail(i: IDiff): IRequestSpecTrail {
  const kind: string = Object.keys(i)[0];
  // @ts-ignore
  return i[kind]!.requestsTrail;
}
