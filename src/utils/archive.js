// 右键「解压」认得的格式，要和 Rust 端 remote_exec/archive.rs 的 detect_archive 保持一致
const ARCHIVE_SUFFIXES = [
  ".tar.gz",
  ".tgz",
  ".tar.bz2",
  ".tbz2",
  ".tbz",
  ".tar.xz",
  ".txz",
  ".tar",
  ".zip",
  ".gz",
];

export const COMPRESS_FORMATS = Object.freeze([
  { format: "zip", extension: ".zip" },
  { format: "tar.gz", extension: ".tar.gz" },
]);

// 返回去掉压缩后缀的名字；不是认得的压缩包返回 null
export function archiveStem(name) {
  const lower = String(name || "").toLowerCase();
  const suffix = ARCHIVE_SUFFIXES.find((candidate) => lower.endsWith(candidate) && lower.length > candidate.length);
  return suffix ? name.slice(0, -suffix.length) : null;
}

// 单个 .gz 解出来是一个文件，不是目录，「解压到同名文件夹」对它没有意义
export function isSingleFileGzip(name) {
  const lower = String(name || "").toLowerCase();
  return lower.endsWith(".gz") && !lower.endsWith(".tar.gz");
}

// 压缩包默认名：选了一项就用它的名字，多项用所在目录名，根目录下叫 archive
export function defaultArchiveName(entries, directory) {
  if (entries.length === 1) return entries[0].name;
  const folder = String(directory || "").split("/").filter(Boolean).pop();
  return folder || "archive";
}
