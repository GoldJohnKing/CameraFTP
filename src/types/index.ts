export interface ServerInfo {
  is_running: boolean;
  ip: string;
  port: number;
  url: string;
  username: string;
  password_info: string;
}

export interface ServerStatus {
  is_running: boolean;
  connected_clients: number;
  files_received: number;
  bytes_received: number;
  last_file: string | null;
}

export interface AppConfig {
  save_path: string;
  auto_open: boolean;
  auto_open_program: string | null;
  port: number;
  auto_select_port: boolean;
  file_extensions: string[];
}

export interface NetworkInterface {
  name: string;
  ip: string;
  is_wifi: boolean;
  is_ethernet: boolean;
  is_up: boolean;
}

// Android JS Bridge 接口
interface FileUploadAndroid {
  onFileUploaded: (path: string | null, size: number) => void;
}

interface SAFPickerAndroid {
  openAllFilesAccessSettings: () => void;
}

declare global {
  interface Window {
    FileUploadAndroid?: FileUploadAndroid;
    SAFPickerAndroid?: SAFPickerAndroid;
  }
}

export {};