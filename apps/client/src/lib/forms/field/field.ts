export interface ControlLinks {
  id: string;
  'aria-describedby': string | undefined;
  'aria-invalid': true | undefined;
}

export function controlLinks(id: string, hasConstraint: boolean, hasError: boolean): ControlLinks {
  const described = [hasConstraint ? `${id}-constraint` : '', hasError ? `${id}-error` : ''].filter((part) => part !== '');
  return {
    id: `${id}-control`,
    'aria-describedby': described.length > 0 ? described.join(' ') : undefined,
    'aria-invalid': hasError ? true : undefined,
  };
}
