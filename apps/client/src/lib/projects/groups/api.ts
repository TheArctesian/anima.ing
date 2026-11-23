import { graphql } from '$lib/hub/generated';
export const GROUPS = graphql(`
  query SharedProjectGroups {
    projectGroups { id name projectIds }
  }
`);
export const CREATE = graphql(`
  mutation CreateSharedProjectGroup($name: String!, $projects: [ID!]!) {
    createProjectGroup(name: $name, projects: $projects) { id }
  }
`);
export const RENAME = graphql(`
  mutation RenameSharedProjectGroup($id: ID!, $name: String!) {
    renameProjectGroup(id: $id, name: $name) { id }
  }
`);
export const MOVE = graphql(`
  mutation MoveSharedProjects($projects: [ID!]!, $group: ID) {
    setProjectsGroup(projects: $projects, group: $group)
  }
`);
export const DELETE = graphql(`
  mutation DeleteSharedProjectGroup($id: ID!) {
    deleteProjectGroup(id: $id)
  }
`);

export const IMPORT = graphql(`
  mutation ImportDeviceProjectGroup($input: ProjectGroupImportInput!) {
    importProjectGroup(input: $input)
  }
`);
