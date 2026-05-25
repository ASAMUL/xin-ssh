export const connections = [
  {
    id: "prod-web",
    name: "Production Web",
    host: "root@192.168.1.10",
    status: "online",
    accent: "#2563eb",
    icon: "i-lucide-globe-2",
  },
  {
    id: "db-server",
    name: "DB Server",
    host: "ubuntu@192.168.1.20",
    status: "online",
    accent: "#7c3aed",
    icon: "i-lucide-database",
  },
  {
    id: "staging",
    name: "Staging Server",
    host: "deploy@10.0.0.45",
    status: "warning",
    accent: "#0891b2",
    icon: "i-lucide-server",
  },
  {
    id: "file-server",
    name: "File Server",
    host: "sftp@files.example.com",
    status: "online",
    accent: "#f59e0b",
    icon: "i-lucide-folder",
  },
  {
    id: "dev-server",
    name: "Dev Server",
    host: "root@dev.local",
    status: "offline",
    accent: "#475569",
    icon: "i-lucide-terminal",
  },
];

export const sessions = [
  {
    id: "prod-web",
    connectionId: "prod-web",
    name: "Production Web",
    status: "online",
    path: "/var/www/html",
  },
  {
    id: "db-server",
    connectionId: "db-server",
    name: "DB Server",
    status: "online",
    path: "/home/ubuntu",
  },
  {
    id: "dev-server",
    connectionId: "dev-server",
    name: "Dev Server",
    status: "offline",
    path: "/srv/app",
  },
];

export const pathCrumbs = [
  { id: "root", label: "root" },
  { id: "var", label: "var" },
  { id: "www", label: "www" },
  { id: "html", label: "html" },
];

export const fileEntries = [
  {
    id: "assets",
    name: "assets",
    size: "--",
    type: "Folder",
    modified: "May 12, 2025 10:24",
    icon: "i-lucide-folder",
    tone: "blue",
    permission: "drwxr-xr-x",
  },
  {
    id: "images",
    name: "images",
    size: "--",
    type: "Folder",
    modified: "May 12, 2025 10:24",
    icon: "i-lucide-folder",
    tone: "sky",
    permission: "drwxr-xr-x",
  },
  {
    id: "index-html",
    name: "index.html",
    size: "2.45 KB",
    type: "HTML Document",
    modified: "May 12, 2025 09:41",
    icon: "i-lucide-file-code-2",
    tone: "orange",
    permission: "-rw-r--r--",
  },
  {
    id: "styles-css",
    name: "styles.css",
    size: "12.48 KB",
    type: "CSS Style Sheet",
    modified: "May 12, 2025 09:41",
    icon: "i-lucide-file-type-2",
    tone: "blue",
    permission: "-rw-r--r--",
  },
  {
    id: "app-js",
    name: "app.js",
    size: "8.62 KB",
    type: "JavaScript File",
    modified: "May 12, 2025 09:41",
    icon: "i-lucide-file-json-2",
    tone: "amber",
    permission: "-rw-r--r--",
  },
  {
    id: "config-json",
    name: "config.json",
    size: "1.21 KB",
    type: "JSON Document",
    modified: "May 12, 2025 09:41",
    icon: "i-lucide-braces",
    tone: "violet",
    permission: "-rw-r--r--",
  },
];

export const terminalLines = [
  { id: "line-1", kind: "prompt", value: "root@192.168.1.10:~# ls -la /var/www/html" },
  { id: "line-2", kind: "text", value: "total 48" },
  { id: "line-3", kind: "text", value: "drwxr-xr-x 5 root root 4096 May 12 10:24 ." },
  { id: "line-4", kind: "text", value: "drwxr-xr-x 3 root root 4096 May 12 09:15 .." },
  { id: "line-5", kind: "directory", value: "drwxr-xr-x 2 root root 4096 May 12 10:24 assets" },
  { id: "line-6", kind: "directory", value: "drwxr-xr-x 2 root root 4096 May 12 10:24 images" },
  { id: "line-7", kind: "text", value: "-rw-r--r-- 1 root root  2450 May 12 09:41 index.html" },
  { id: "line-8", kind: "text", value: "-rw-r--r-- 1 root root 12485 May 12 09:41 styles.css" },
  { id: "line-9", kind: "text", value: "-rw-r--r-- 1 root root  8623 May 12 09:41 app.js" },
  { id: "line-10", kind: "text", value: "-rw-r--r-- 1 root root  1210 May 12 09:41 config.json" },
  { id: "line-11", kind: "prompt", value: "root@192.168.1.10:~# " },
];

export const transferSummary = {
  uploadSpeed: "12.0 MB/s",
  downloadSpeed: "1.2 MB/s",
  progress: 68,
  ping: "12ms",
  protocol: "SFTP",
};

export const transferTasks = [
  {
    id: "upload-1",
    direction: "upload",
    label: "Uploading 2 files",
    speed: "12.0 MB/s",
  },
  {
    id: "download-1",
    direction: "download",
    label: "Downloading 1 file",
    speed: "1.2 MB/s",
  },
];
