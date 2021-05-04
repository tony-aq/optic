export const linkToCapture = (
  uiBaseUrl: string,
  sessionId: string,
  captureId: string
) => ``;
export const linkToDiffs = (uiBaseUrl: string, sessionId: string) =>
  `${uiBaseUrl}/apis/${sessionId}/diffs`;
export const linkToDocumentation = (uiBaseUrl: string, sessionId: string) =>
  `${uiBaseUrl}/apis/${sessionId}/documentation`;

export const linkToSetup = (sessionId: string) => `https://useoptic.com/docs`;
