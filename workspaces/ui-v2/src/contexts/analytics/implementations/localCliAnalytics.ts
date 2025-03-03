import { Client } from '@useoptic/cli-client';
import * as Sentry from '@sentry/react';
import * as FullStory from '@fullstory/browser';
import { LogLevel } from '@sentry/types';
import { AnalyticsStoreProps } from '../AnalyticsStore';
import { getOrSetAgentFromLocalStorage } from '../utils';

declare global {
  interface Window {
    Intercom?: (...args: any) => any;
  }
}

const packageJson = require('../../../../package.json');
const clientId = `local_cli_${packageJson.version}`;

const fetchIdentity = async () => {
  const cliClient = new Client('/api');
  try {
    const response = await cliClient.getIdentity();
    if (response.ok) {
      const { anonymousId } = await response.json();
      return anonymousId;
    } else {
      throw new Error();
    }
  } catch (e) {
    return getOrSetAgentFromLocalStorage('anon_id');
  }
};

export const getMetadata: (
  apiNameFetcher: () => Promise<string>
) => AnalyticsStoreProps['getMetadata'] = (apiNameFetcher) => async () => {
  const clientAgent = await fetchIdentity();
  const apiName = await apiNameFetcher();

  return {
    clientId,
    clientAgent,
    apiName,
  };
};

export const initialize: AnalyticsStoreProps['initialize'] = async (
  metadata,
  appConfig
) => {
  if (!appConfig.analytics.enabled) {
    return;
  }

  if (appConfig.analytics.fullStoryOrgId) {
    FullStory.init({ orgId: appConfig.analytics.fullStoryOrgId });
    FullStory.setUserVars({
      clientId: clientId,
      anonId: metadata.clientAgent,
      apiName: metadata.apiName,
    });
    FullStory.event('Session', {
      clientId: clientId,
      anonId: metadata.clientAgent,
      apiName: metadata.apiName,
    });
  }

  if (appConfig.analytics.sentryUrl) {
    Sentry.init({
      dsn: appConfig.analytics.sentryUrl,
      release: clientId,
      logLevel: LogLevel.Debug,
      normalizeDepth: 10,
    });
    Sentry.setUser({ id: metadata.clientAgent });
  }

  if (window.Intercom && appConfig.analytics.intercomAppId) {
    window.Intercom('boot', {
      app_id: appConfig.analytics.intercomAppId,
      user_id: metadata.clientAgent,
    });
    window.Intercom('hide');
  }
};

export const track: AnalyticsStoreProps['track'] = async (event, metadata) => {
  const cliClient = new Client('/api');

  cliClient.postTrackingEvents([
    {
      type: event.type,
      data: { ...event.data, specId: metadata.specId },
    },
  ]);
  try {
    FullStory.event(event.type, { ...event.data, specId: metadata.specId });
  } catch (e) {}
};
