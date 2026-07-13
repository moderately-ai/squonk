// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

export const nativePackages = [
  { packageName: "@squonk-sql/native-darwin-arm64", target: "aarch64-apple-darwin", os: "darwin", cpu: "arm64" },
  { packageName: "@squonk-sql/native-darwin-x64", target: "x86_64-apple-darwin", os: "darwin", cpu: "x64" },
  { packageName: "@squonk-sql/native-linux-arm64-gnu", target: "aarch64-unknown-linux-gnu", os: "linux", cpu: "arm64", libc: "glibc" },
  { packageName: "@squonk-sql/native-linux-x64-gnu", target: "x86_64-unknown-linux-gnu", os: "linux", cpu: "x64", libc: "glibc" },
  { packageName: "@squonk-sql/native-linux-arm64-musl", target: "aarch64-unknown-linux-musl", os: "linux", cpu: "arm64", libc: "musl" },
  { packageName: "@squonk-sql/native-linux-x64-musl", target: "x86_64-unknown-linux-musl", os: "linux", cpu: "x64", libc: "musl" },
  { packageName: "@squonk-sql/native-win32-arm64-msvc", target: "aarch64-pc-windows-msvc", os: "win32", cpu: "arm64" },
  { packageName: "@squonk-sql/native-win32-x64-msvc", target: "x86_64-pc-windows-msvc", os: "win32", cpu: "x64" },
];

export function currentNativePackage() {
  const cpu = process.arch === "x64" || process.arch === "arm64" ? process.arch : null;
  if (cpu === null) return null;
  if (process.platform === "darwin") return nativePackages.find((item) => item.os === "darwin" && item.cpu === cpu) ?? null;
  if (process.platform === "win32") return nativePackages.find((item) => item.os === "win32" && item.cpu === cpu) ?? null;
  if (process.platform === "linux") {
    const glibc = process.report?.getReport()?.header?.glibcVersionRuntime;
    const libc = glibc ? "glibc" : "musl";
    return nativePackages.find((item) => item.os === "linux" && item.cpu === cpu && item.libc === libc) ?? null;
  }
  return null;
}
