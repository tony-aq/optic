import React, { FC } from 'react';
import { createFlatList } from '<src>/components/FieldOrParameter';
import { useShapeDescriptor } from '<src>/hooks/useShapeDescriptor';
import { IShapeRenderer } from '<src>/components';

type ContributionFetcherProps = {
  rootShapeId: string;
  endpointId: string;
  // TODO change this typing
  children: (
    contributions: {
      id: string;
      contributionKey: string;
      value: string;
      endpointId: string;
      depth: number;
      name: string;
      shapes: IShapeRenderer[];
    }[]
  ) => React.ReactNode;
};

// TODO replace this by fetching the contributions in redux
export const ContributionFetcher: FC<ContributionFetcherProps> = ({
  rootShapeId,
  endpointId,
  children,
}) => {
  const shapes = useShapeDescriptor(rootShapeId, undefined);
  const contributions = createFlatList(shapes);
  const contributionsMapped = contributions.map((contribution) => ({
    id: contribution.contributionId,
    contributionKey: 'description',
    value: contribution.description,
    endpointId: endpointId,
    depth: contribution.depth,
    name: contribution.name,
    shapes: contribution.shapes,
  }));

  return <>{children(contributionsMapped)}</>;
};

type ShapeFetcherProps = {
  rootShapeId: string;
  children: (shapes: ReturnType<typeof useShapeDescriptor>) => React.ReactNode;
};

// TODO replace this by fetching the shapes in redux
export const ShapeFetcher: FC<ShapeFetcherProps> = ({
  rootShapeId,
  children,
}) => {
  const shapes = useShapeDescriptor(rootShapeId, undefined);
  return <>{children(shapes)}</>;
};
