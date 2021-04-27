import * as React from 'react';
import { useMemo } from 'react';
import { NavigationRoute } from '../../navigation/NavigationRoute';
import {
  useDocumentationPageLink,
  useEndpointPageLink,
} from '../../navigation/Routes';
import groupBy from 'lodash.groupby';
import { CenteredColumn } from '../../layouts/CenteredColumn';
import { IEndpoint, useEndpoints } from '../../hooks/useEndpointsHook';
import { List, ListItem, Typography } from '@material-ui/core';
import { EndpointName } from '../../common';
import { EndpointNameMiniContribution } from '../../documentation/Contributions';
import {
  ContributionEditingStore,
  useContributionEditing,
} from '../../hooks/edit/Contributions';
import { EditContributionsButton } from '../../hooks/edit/EditContributionsButton';
import { FullWidth } from '../../layouts/FullWidth';
import { EndpointNameContribution } from '../../documentation/Contributions';
import { MarkdownBodyContribution } from '../../documentation/MarkdownBodyContribution';
import { TwoColumn } from '../../documentation/TwoColumn';
import { useHistory } from 'react-router-dom';
import { PathParametersViewEdit } from '../../documentation/PathParameters';
import { EndpointTOC } from '../../documentation/EndpointTOC';
import { useEndpointBody } from '../../hooks/useEndpointBodyHook';
import { CodeBlock } from '../../documentation/BodyRender';
import { SubtleBlueBackground } from '../../theme';
import { TwoColumnBody } from '../../documentation/RenderBody';
import { getEndpointId } from '../../utilities/endpoint-utilities';
import { Loading } from '../../loaders/Loading';
import { ChangesSinceDropdown } from '../../changelog/ChangelogDropdown';
import { PromptNavigateAway } from '../../PromptNavigateAway';
import { useBaseUrl } from '../../hooks/useBaseUrl';
import { useAppConfig } from '../../hooks/config/AppConfiguration';
import { useEndpointsChangelog } from '../../hooks/useEndpointsChangelog';
import { useChangelogStyles } from '../../changelog/ChangelogBackground';

export function DocumentationPages(props: any) {
  const documentationPageLink = useDocumentationPageLink();
  const endpointPageLink = useEndpointPageLink();
  const history = useHistory();
  const baseUrl = useBaseUrl();

  const onEndpointClicked = (pathId: string, method: string) => {
    history.push(endpointPageLink.linkTo(pathId, method));
  };

  return (
    <ContributionEditingStore>
      <>
        <NavigationRoute
          path={baseUrl}
          Component={(props: any) => (
            <DocumentationRootPage
              {...props}
              onEndpointClicked={onEndpointClicked}
            />
          )}
          AccessoryNavigation={DocsPageAccessoryNavigation}
        />
        <NavigationRoute
          path={documentationPageLink.path}
          Component={(props: any) => (
            <DocumentationRootPage
              {...props}
              onEndpointClicked={onEndpointClicked}
            />
          )}
          AccessoryNavigation={DocsPageAccessoryNavigation}
        />
        <NavigationRoute
          path={endpointPageLink.path}
          Component={EndpointRootPage}
          AccessoryNavigation={DocsPageAccessoryNavigation}
        />
      </>
    </ContributionEditingStore>
  );
}

export function DocsPageAccessoryNavigation(props: any) {
  const appConfig = useAppConfig();
  const { isEditing, pendingCount } = useContributionEditing();

  return (
    <div style={{ paddingRight: 10, display: 'flex', flexDirection: 'row' }}>
      <PromptNavigateAway shouldPrompt={isEditing && pendingCount > 0} />
      {appConfig.navigation.showChangelog && <ChangesSinceDropdown />}
      {appConfig.documentation.allowDescriptionEditing && (
        <EditContributionsButton />
      )}
    </div>
  );
}

export function DocumentationRootPage(props: {
  onEndpointClicked: (pathId: string, method: string) => void;
  changelogBatchId?: string;
}) {
  const { endpoints, loading } = useEndpoints(props.changelogBatchId);

  const grouped = useMemo(() => groupBy(endpoints, 'group'), [endpoints]);
  const tocKeys = Object.keys(grouped).sort();
  const changelogStyles = useChangelogStyles();

  // const history = useHistory();
  // const endpointPageLink = useEndpointPageLink();

  if (loading) {
    return <Loading />;
  }

  return (
    <CenteredColumn maxWidth="md" style={{ marginTop: 35 }}>
      <List dense>
        {tocKeys.map((tocKey) => {
          return (
            <div key={tocKey}>
              <Typography
                variant="subtitle2"
                style={{ fontFamily: 'Ubuntu Mono' }}
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
                      props.onEndpointClicked(endpoint.pathId, endpoint.method)
                    }
                    className={endpoint.changelog?.added ? changelogStyles.added : ""}
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

export function EndpointRootPage(props: any) {
  const { endpoints, loading } = useEndpoints();

  const { match } = props;
  const { pathId, method } = match.params;

  const bodies = useEndpointBody(pathId, method);

  const thisEndpoint = useMemo(
    () => endpoints.find((i) => i.pathId === pathId && i.method === method),
    [pathId, method, endpoints]
  );

  if (loading) {
    return <Loading />;
  }

  if (!thisEndpoint) {
    return <>no endpoint here</>;
  }
  const endpointId = getEndpointId({ method, pathId });

  return (
    <FullWidth style={{ paddingTop: 30, paddingBottom: 400 }}>
      <EndpointNameContribution
        id={endpointId}
        contributionKey="purpose"
        defaultText="What does this endpoint do?"
        initialValue={thisEndpoint.purpose}
      />
      <EndpointName
        fontSize={19}
        leftPad={0}
        method={thisEndpoint.method}
        fullPath={thisEndpoint.fullPath}
      />
      <TwoColumn
        style={{ marginTop: 5 }}
        left={
          <MarkdownBodyContribution
            id={endpointId}
            contributionKey={'description'}
            defaultText={'Describe this endpoint'}
            initialValue={thisEndpoint.description}
          />
        }
        right={
          <CodeBlock
            header={
              <EndpointName
                fontSize={14}
                leftPad={0}
                method={thisEndpoint.method}
                fullPath={thisEndpoint.fullPath}
              />
            }
          >
            <PathParametersViewEdit parameters={thisEndpoint.pathParameters} />
            <div
              style={{
                marginTop: 10,
                backgroundColor: SubtleBlueBackground,
                borderTop: '1px solid #e2e2e2',
              }}
            >
              <EndpointTOC
                requests={bodies.requests}
                responses={bodies.responses}
              />
            </div>
          </CodeBlock>
        }
      />

      {bodies.requests.map((i, index) => {
        return (
          <TwoColumnBody
            key={index}
            rootShapeId={i.rootShapeId}
            bodyId={i.requestId}
            location={'Request Body Parameters'}
            description={i.description}
          />
        );
      })}
      {bodies.responses.map((i, index) => {
        return (
          <TwoColumnBody
            key={index}
            rootShapeId={i.rootShapeId}
            bodyId={i.responseId}
            location={`${i.statusCode} Response`}
            description={i.description}
          />
        );
      })}
    </FullWidth>
  );
}
