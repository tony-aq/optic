import fs from 'fs-extra';
import path from 'path';
//@ts-ignore
import oboe from 'oboe';
import { CaptureSaver } from './capture-saver';
import { IHttpInteraction } from '@useoptic/optic-domain';

async function main(
  inputFilePaths: {
    events: string;
    interactions: string;
  },
  outputBaseDirectory: string,
  captureId: string
) {
  console.log({ inputFilePaths });
  const events: any[] = await fs.readJson(inputFilePaths.events);
  const captureBaseDirectory = path.join(
    outputBaseDirectory,
    '.optic',
    'captures'
  );
  const captureSaver = new CaptureSaver({
    captureBaseDirectory,
    captureId,
  });
  const input = fs.createReadStream(inputFilePaths.interactions);
  await captureSaver.init();
  await new Promise<void>((resolve, reject) => {
    oboe(input)
      .on('node', {
        // @ts-ignore
        '!.*': function (sample: IHttpInteraction) {
          console.count('sample');
          console.log({ sample });
          captureSaver.save(sample);
        },
      })
      .on('done', function () {
        console.log('done');
        resolve();
      })
      .on('fail', function (e: any) {
        console.error(e);
        reject(e);
      });
  });

  const files = [
    {
      location: path.join(outputBaseDirectory, 'optic.yml'),
      contents: `name: ${JSON.stringify(path.basename(inputFilePaths.events))}`,
    },
    {
      location: path.join(
        outputBaseDirectory,
        '.optic',
        'api',
        'specification.json'
      ),
      contents: JSON.stringify(events),
    },
    {
      location: path.join(
        outputBaseDirectory,
        '.optic',
        'captures',
        captureId,
        'optic-capture-state.json'
      ),
      contents: JSON.stringify({
        captureId,
        status: 'completed',
        metadata: {
          startedAt: new Date().toISOString(),
          taskConfig: null,
          lastInteraction: null,
        },
      }),
    },
  ];

  await Promise.all(
    files.map(async (x) => {
      const { location, contents } = x;
      await fs.ensureDir(path.dirname(location));
      return fs.writeFile(location, contents);
    })
  );
}

const [
  ,
  ,
  inputEventsFilePath,
  inputInteractionsFilePath,
  outputBaseDirectory,
  captureId,
] = process.argv;
main(
  { interactions: inputInteractionsFilePath, events: inputEventsFilePath },
  outputBaseDirectory,
  captureId
);
