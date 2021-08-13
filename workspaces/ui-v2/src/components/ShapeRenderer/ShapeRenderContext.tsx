import * as React from 'react';
import { useContext, useState, useCallback } from 'react';
import { DepthStore } from './DepthContext';
import { OneOfTabsProps } from './OneOfTabs';

type IShapeRenderContext = {
  selectedFieldId?: string | null;
  showExamples: boolean;
  fieldsAreSelectable: boolean;
  getChoice: (branch: OneOfTabsProps) => string;
  updateChoice: (shapeId: string, branchId: string) => void;
  selectField: (fieldId: string) => void;
};

export const ShapeRenderContext = React.createContext<IShapeRenderContext | null>(
  null
);

type ShapeRenderContextProps = {
  children: React.ReactNode;
  showExamples: boolean;
  selectedFieldId?: string | null;
  fieldsAreSelectable?: boolean;
  setSelectedField?: (fieldId: string) => void;
};

export const ShapeRenderStore = ({
  children,
  showExamples,
  selectedFieldId,
  fieldsAreSelectable,
  setSelectedField,
}: ShapeRenderContextProps) => {
  const [selectedOneOfChoices, updateSelectedOneOfChoices] = useState<{
    [key: string]: string;
  }>({});
  console.log(selectedOneOfChoices);

  const getChoice = (branch: OneOfTabsProps) => {
    if (selectedOneOfChoices[branch.shapeId]) {
      return selectedOneOfChoices[branch.shapeId];
    } else {
      return branch.choices[0].id;
    }
  };

  const updateChoice = (parentShapeId: string, branchId: string) => {
    updateSelectedOneOfChoices(
      (previousChoices: { [key: string]: string }) => ({
        ...previousChoices,
        [parentShapeId]: branchId,
      })
    );
  };

  const selectField = useCallback(
    (fieldId) => {
      if (setSelectedField) setSelectedField(fieldId);
    },
    [setSelectedField]
  );

  return (
    <ShapeRenderContext.Provider
      value={{
        showExamples,
        getChoice,
        updateChoice,
        selectedFieldId,
        selectField,
        fieldsAreSelectable: !!fieldsAreSelectable,
      }}
    >
      <DepthStore depth={0}>{children}</DepthStore>
    </ShapeRenderContext.Provider>
  );
};

export function useShapeRenderContext() {
  const value = useContext(ShapeRenderContext);
  if (!value) {
    throw new Error('Could not find ShapeRendererContext');
  }

  return value;
}
