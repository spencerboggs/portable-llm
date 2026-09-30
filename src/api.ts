import { invoke } from "@tauri-apps/api/core";
import type {
  AppBootstrap,
  Conversation,
  DriveInfo,
  HardwareInfo,
  KnowledgeFileInfo,
  RuntimeInfo,
  ScriptInfo,
  Settings,
  SpaceRequirement,
} from "./types";

export const api = {
  getBootstrap: () => invoke<AppBootstrap>("get_bootstrap"),
  refreshDrives: () => invoke<DriveInfo[]>("refresh_drives"),
  getHardware: () => invoke<HardwareInfo>("get_hardware"),
  getSpaceRequirement: (driveLetter: string) =>
    invoke<SpaceRequirement>("get_space_requirement", { driveLetter }),
  getRuntime: () => invoke<RuntimeInfo>("get_runtime"),
  loadModel: (driveLetter: string) => invoke<RuntimeInfo>("load_model", { driveLetter }),
  resumeExisting: () => invoke<RuntimeInfo>("resume_existing"),
  removeModel: () => invoke<RuntimeInfo>("remove_model"),
  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (patch: Settings) => invoke<Settings>("update_settings", { patch }),
  listConversations: () => invoke<Conversation[]>("list_conversations"),
  getConversation: (id: string) => invoke<Conversation>("get_conversation", { id }),
  createConversation: (title?: string) =>
    invoke<Conversation>("create_conversation", { title: title ?? null }),
  deleteConversation: (id: string) => invoke<void>("delete_conversation", { id }),
  sendChat: (request: {
    conversationId?: string | null;
    message: string;
    regenerate: boolean;
  }) =>
    invoke<{ conversation: Conversation }>("send_chat", {
      request: {
        conversationId: request.conversationId ?? null,
        message: request.message,
        regenerate: request.regenerate,
      },
    }),
  stopGeneration: () => invoke<void>("stop_generation"),
  reloadKnowledge: () => invoke<number>("reload_knowledge"),
  listKnowledge: () => invoke<KnowledgeFileInfo[]>("list_knowledge"),
  readKnowledgeFile: (relativePath: string) =>
    invoke<string>("read_knowledge_file", { relativePath }),
  writeKnowledgeFile: (relativePath: string, content: string) =>
    invoke<void>("write_knowledge_file", { relativePath, content }),
  deleteKnowledgeFile: (relativePath: string) =>
    invoke<void>("delete_knowledge_file", { relativePath }),
  listScripts: () => invoke<ScriptInfo[]>("list_scripts"),
  saveScript: (name: string, content: string) =>
    invoke<ScriptInfo>("save_script", { name, content }),
  readScript: (name: string) => invoke<string>("read_script", { name }),
  openScriptsFolder: () => invoke<void>("open_scripts_folder"),
  getUsbRoot: () => invoke<string>("get_usb_root"),
};
