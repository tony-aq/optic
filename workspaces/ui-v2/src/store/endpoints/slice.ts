import { createSlice, SerializedError } from '@reduxjs/toolkit';

import { AsyncStatus, IEndpoint } from '<src>/types';

import { fetchEndpoints } from './thunks';

const initialState: {
  results: AsyncStatus<IEndpoint[], SerializedError>;
} = {
  results: {
    loading: true,
  },
};

const endpointsSlice = createSlice({
  name: 'endpoints',
  initialState,
  reducers: {},
  extraReducers: (builder) => {
    builder.addCase(fetchEndpoints.fulfilled, (state, action) => {
      const results = action.payload;

      state.results = {
        loading: false,
        data: results,
      };
    });
    builder.addCase(fetchEndpoints.rejected, (state, action) => {
      state.results = {
        loading: false,
        error: action.error,
      };
    });
  },
});

export const actions = {
  fetchEndpoints,
};
export const reducer = endpointsSlice.reducer;
