export type LoadStatus = "notLoaded" | "loading" | "running" | "removing" | "error";

export interface DriveInfo {
  letter: string;
  name: string;
  totalBytes: number;
  freeBytes: number;
  driveType: string;
  mediaType: string;
  isReady: boolean;
  isRemovable: boolean;
}

export interface HardwareInfo {
  cpu: string;
  ramBytes: number;
  gpu: string;
  vramBytes: number | null;
  gpuAccelerationAvailable: boolean;
  note: string;
}

export interface RuntimeInfo {
  status: LoadStatus;
  targetDrive: string | null;
  stagingPath: string | null;
  modelName: string;
  progressMessage: string;
  progressPercent: number;
  error: string | null;
  ollamaBaseUrl: string | null;
  usingGpu: boolean | null;
}

export interface Settings {
  preferredDrive: string | null;
  internetEnabled: boolean;
  portableMode: boolean;
  theme: string;
  modelName: string;
  ollamaPort: number;
}

export interface SpaceRequirement {
  requiredBytes: number;
  runtimeBytes: number;
  modelBytes: number;
  otherBytes: number;
  availableBytes: number;
  enoughSpace: boolean;
  missingRuntime: boolean;
  missingModel: boolean;
}

export interface AppBootstrap {
  usbRoot: string;
  runtime: RuntimeInfo;
  settings: Settings;
  hardware: HardwareInfo;
  drives: DriveInfo[];
  modelLabel: string;
  requiredSpaceHint: string;
}

export interface ChatMessage {
  id: string;
  role: string;
  content: string;
  createdAt: string;
}

export interface Conversation {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  messages: ChatMessage[];
}

export interface ChatStreamEvent {
  conversationId: string;
  messageId: string;
  delta: string;
  done: boolean;
  error: string | null;
}

export interface KnowledgeFileInfo {
  relativePath: string;
  sizeBytes: number;
}

export interface ScriptInfo {
  name: string;
  path: string;
  sizeBytes: number;
}

export type ViewId = "dashboard" | "chat" | "knowledge" | "scripts" | "settings";

export function formatBytes(bytes: number): string {
  const kb = 1024;
  const mb = kb * 1024;
  const gb = mb * 1024;
  const tb = gb * 1024;
  if (bytes >= tb) return `${(bytes / tb).toFixed(1)} TB`;
  if (bytes >= gb) return `${(bytes / gb).toFixed(1)} GB`;
  if (bytes >= mb) return `${(bytes / mb).toFixed(0)} MB`;
  if (bytes >= kb) return `${(bytes / kb).toFixed(0)} KB`;
  return `${bytes} B`;
}
