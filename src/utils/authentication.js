export function usesSavedCredentials(authMethod) {
  return authMethod === "password" || authMethod === "privateKey";
}
