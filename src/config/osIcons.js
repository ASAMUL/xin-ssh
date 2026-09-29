// 连接卡片上的系统图标目录。
// 这个文件同时被 vite.config.js 读取（只把这里用到的 devicon 打进包——完整集合 5MB），
// 所以不能 import 任何浏览器侧模块。
//
// id 会写进 profiles.json（osIcon / detectedOs），发版后只能加不能改名。

export const OS_ICON_AUTO = "auto";

// mono：devicon 里没有自带颜色的单色标（苹果）和 lucide 线性图标，渲染时跟随文字色，
// 否则在深色底座上会整块消失。
export const OS_ICON_GROUPS = [
  {
    id: "linux",
    items: [
      { id: "linux", label: "Linux", icon: "i-devicon-linux" },
      { id: "ubuntu", label: "Ubuntu", icon: "i-devicon-ubuntu" },
      { id: "debian", label: "Debian", icon: "i-devicon-debian" },
      { id: "centos", label: "CentOS", icon: "i-devicon-centos" },
      { id: "redhat", label: "Red Hat", icon: "i-devicon-redhat" },
      { id: "fedora", label: "Fedora", icon: "i-devicon-fedora" },
      { id: "rocky", label: "Rocky", icon: "i-devicon-rockylinux" },
      { id: "alma", label: "AlmaLinux", icon: "i-devicon-almalinux" },
      { id: "arch", label: "Arch", icon: "i-devicon-archlinux" },
      { id: "artix", label: "Artix", icon: "i-devicon-artixlinux" },
      { id: "opensuse", label: "openSUSE", icon: "i-devicon-opensuse" },
      { id: "gentoo", label: "Gentoo", icon: "i-devicon-gentoo" },
      { id: "kali", label: "Kali", icon: "i-devicon-kalilinux" },
      { id: "mint", label: "Mint", icon: "i-devicon-linuxmint" },
      { id: "nixos", label: "NixOS", icon: "i-devicon-nixos" },
      { id: "raspberrypi", label: "Raspberry Pi", icon: "i-devicon-raspberrypi" },
    ],
  },
  {
    id: "system",
    items: [
      { id: "macos", label: "macOS", icon: "i-devicon-apple", mono: true },
      { id: "windows", label: "Windows", icon: "i-devicon-windows11" },
      { id: "unix", label: "Unix / BSD", icon: "i-devicon-unix" },
      { id: "android", label: "Android", icon: "i-devicon-android" },
    ],
  },
  {
    id: "platform",
    items: [
      { id: "proxmox", label: "Proxmox", icon: "i-devicon-proxmox" },
      { id: "docker", label: "Docker", icon: "i-devicon-docker" },
      { id: "kubernetes", label: "Kubernetes", icon: "i-devicon-kubernetes" },
      { id: "hyperv", label: "Hyper-V", icon: "i-devicon-hyperv" },
      { id: "aws", label: "AWS", icon: "i-devicon-amazonwebservices" },
      { id: "azure", label: "Azure", icon: "i-devicon-azure" },
      { id: "gcp", label: "Google Cloud", icon: "i-devicon-googlecloud" },
      { id: "digitalocean", label: "DigitalOcean", icon: "i-devicon-digitalocean" },
      { id: "oracle", label: "Oracle", icon: "i-devicon-oracle" },
    ],
  },
  {
    // devicon 没有群晖 / 路由器这类设备标，用 lucide 线性图标补位；名称走 i18n
    id: "device",
    items: [
      { id: "server", labelKey: "osIcon.names.server", icon: "i-lucide-server", mono: true },
      { id: "nas", labelKey: "osIcon.names.nas", icon: "i-lucide-hard-drive", mono: true },
      { id: "router", labelKey: "osIcon.names.router", icon: "i-lucide-router", mono: true },
    ],
  },
];

const OS_ICONS = new Map(OS_ICON_GROUPS.flatMap((group) => group.items).map((item) => [item.id, item]));

// 未选图标、自动识别尚未拿到结果时的占位：一台中性的服务器，不猜发行版
export const FALLBACK_OS_ICON = OS_ICONS.get("server");

export const DEVICON_NAMES = [...OS_ICONS.values()]
  .map((item) => item.icon)
  .filter((icon) => icon.startsWith("i-devicon-"))
  .map((icon) => icon.slice("i-devicon-".length));

export function getOsIcon(id) {
  return OS_ICONS.get(id) ?? null;
}

// 手动选择优先；「自动」时用连接后识别到的结果，都没有就是中性占位
export function resolveProfileOsIcon(profile) {
  const chosen = profile?.osIcon && profile.osIcon !== OS_ICON_AUTO ? getOsIcon(profile.osIcon) : null;
  return chosen ?? getOsIcon(profile?.detectedOs) ?? FALLBACK_OS_ICON;
}

export function isAutoOsIcon(profile) {
  return !profile?.osIcon || profile.osIcon === OS_ICON_AUTO || !getOsIcon(profile.osIcon);
}

// /etc/os-release 的 ID → 图标。只做精确映射、不按 ID_LIKE 猜：
// Pop!_OS、Manjaro 这类衍生版挂上游的标反而会让人认错机器，宁可显示通用 Tux。
const OS_RELEASE_IDS = {
  ubuntu: "ubuntu",
  debian: "debian",
  centos: "centos",
  rhel: "redhat",
  fedora: "fedora",
  rocky: "rocky",
  almalinux: "alma",
  arch: "arch",
  artix: "artix",
  gentoo: "gentoo",
  kali: "kali",
  linuxmint: "mint",
  nixos: "nixos",
  raspbian: "raspberrypi",
  ol: "oracle",
  amzn: "aws",
};

const UNIX_KERNELS = ["freebsd", "openbsd", "netbsd", "dragonfly", "sunos", "aix"];

export function detectOsIcon({ osId, kernel } = {}) {
  const id = String(osId ?? "").trim().toLowerCase();
  if (OS_RELEASE_IDS[id]) return OS_RELEASE_IDS[id];
  if (id.startsWith("opensuse") || id === "sles" || id === "sled") return "opensuse";

  const kernelName = String(kernel ?? "").trim().split(/\s+/)[0].toLowerCase();
  if (kernelName === "linux") return "linux";
  if (kernelName === "darwin") return "macos";
  if (UNIX_KERNELS.includes(kernelName)) return "unix";
  return null;
}
