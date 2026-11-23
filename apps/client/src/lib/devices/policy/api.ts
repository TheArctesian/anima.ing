import { graphql } from '$lib/hub/generated';

// F-3: the session policy (RA-036) has its own documents, so a hub without them answers every other device operation.
export const DevicePolicy = graphql(`
  query DevicePolicy($id: ID!) {
    device(id: $id) {
      sessionLimit
    }
  }
`);

export const SetDevicePolicy = graphql(`
  mutation SetDevicePolicy($id: ID!, $input: UpdateDeviceInput!) {
    updateDevice(id: $id, input: $input) {
      sessionLimit
    }
  }
`);
