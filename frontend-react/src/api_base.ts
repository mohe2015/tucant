export async function genericFetch(
  url: string,
  input: unknown
): Promise<unknown> {
  const response = await fetch(url, {
    credentials: "include",
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "x-csrf-protection": "tucant",
    },
    body: JSON.stringify(input),
  });
  if (!response.ok) {
    let errorMessage;
    try {
      errorMessage = `${response.status} ${
        response.statusText
      }: ${await response.text()}`;
    } catch (err) {
      errorMessage = `${response.status} ${
        response.statusText
      }, while loading error body: ${String(err)}`;
    }
    throw new Error(errorMessage);
  }
  return (await response.json()) as unknown;
}
