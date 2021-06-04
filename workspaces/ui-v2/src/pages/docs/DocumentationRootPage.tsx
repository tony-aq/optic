import React, { FC, useMemo } from 'react';
import { useHistory, useRouteMatch } from 'react-router-dom';
import groupBy from 'lodash.groupby';
import {
  CenteredColumn,
  EndpointName,
  Loading,
  PageLayout,
} from '<src>/components';
import { Box, List, ListItem, Typography } from '@material-ui/core';
import { useContributionEditing } from './contexts/Contributions';
import { useAppSelector } from '<src>/store';
import { useRunOnKeypress } from '<src>/hooks/util';
import { IEndpoint } from '<src>/types';
import { getEndpointId } from '<src>/utils';
import {
  DocsPageAccessoryNavigation,
  EndpointNameMiniContribution,
} from './components';

export const DocumentationRootPageWithDocsNav: FC = () => (
  <PageLayout AccessoryNavigation={DocsPageAccessoryNavigation}>
    <DocumentationRootPage />
  </PageLayout>
);

export function DocumentationRootPage() {
  const endpointsState = useAppSelector((state) => state.endpoints.results);
  const history = useHistory();
  const match = useRouteMatch();

  const {
    isEditing,
    pendingCount,
    setCommitModalOpen,
  } = useContributionEditing();

  const grouped = useMemo(() => groupBy(endpointsState.data || [], 'group'), [
    endpointsState,
  ]);
  const tocKeys = Object.keys(grouped).sort();
  const onKeyPress = useRunOnKeypress(
    () => {
      if (isEditing && pendingCount > 0) {
        setCommitModalOpen(true);
      }
    },
    {
      keys: new Set(['Enter']),
      inputTagNames: new Set(['input']),
    }
  );

  if (endpointsState.loading) {
    return <Loading />;
  }

  if (tocKeys.length === 0) {
    return (
      <Box
        display="flex"
        height="100%"
        alignItems="center"
        justifyContent="center"
      >
        <Typography
          variant="h6"
          style={{ fontFamily: 'Ubuntu Mono', marginBottom: '25%' }}
        >
          No endpoints have been documented yet
        </Typography>
      </Box>
    );
  }

  return (
    <CenteredColumn maxWidth="md" style={{ marginTop: 35 }}>
      <List dense onKeyPress={onKeyPress}>
        {tocKeys.map((tocKey) => {
          return (
            <div key={tocKey}>
              <Typography
                variant="h6"
                style={{ fontFamily: 'Ubuntu Mono', fontWeight: 600 }}
              >
                {tocKey}
              </Typography>
              {grouped[tocKey].map((endpoint: IEndpoint, index: number) => {
                return (
                  <ListItem
                    key={index}
                    button
                    disableRipple
                    disableGutters
                    style={{ display: 'flex' }}
                    onClick={() =>
                      history.push(
                        `${match.url}/paths/${endpoint.pathId}/methods/${endpoint.method}`
                      )
                    }
                  >
                    <div style={{ flex: 1 }}>
                      <EndpointName
                        method={endpoint.method}
                        fullPath={endpoint.fullPath}
                        leftPad={6}
                      />
                    </div>
                    <div
                      style={{ paddingRight: 15 }}
                      onClick={(e) => e.stopPropagation()}
                    >
                      <EndpointNameMiniContribution
                        id={getEndpointId({
                          method: endpoint.method,
                          pathId: endpoint.pathId,
                        })}
                        defaultText="name for this endpoint"
                        contributionKey="purpose"
                        initialValue={endpoint.purpose}
                      />
                    </div>
                  </ListItem>
                );
              })}
            </div>
          );
        })}
      </List>
    </CenteredColumn>
  );
}
