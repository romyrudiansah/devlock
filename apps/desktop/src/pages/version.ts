/** Utilitas versi kecil untuk UI (`8.4.11` → [8, 4, 11]). */
export const Version = {
  parts(version: string): number[] {
    return version
      .replace(/^v/, "")
      .split(".")
      .map((p) => parseInt(p, 10))
      .filter((n) => !Number.isNaN(n));
  },
};
