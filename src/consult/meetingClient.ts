export interface Binding {
  client_id: string;
  session_id: string;
}

export function shouldStopOnRevoke(
  binding: Binding | null,
  revokedClientId: string,
): boolean {
  return binding !== null && binding.client_id === revokedClientId;
}
