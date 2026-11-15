// Reading a File as text.
//
// `File.text()` is the modern one-liner, but it is not present in every
// environment (older jsdom, some embedded webviews). FileReader is, and it
// is what the plan called for, so this is the single place the read happens.

export function readFileText(file: File): Promise<string> {
  if (typeof file.text === "function") {
    return file.text();
  }

  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result ?? ""));
    reader.onerror = () => reject(reader.error ?? new Error("read failed"));
    reader.readAsText(file);
  });
}
