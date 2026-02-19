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
  file_extensions: string[];
}

export interface NetworkInterface {
  name: string;
  ip: string;
  is_wifi: boolean;
  is_ethernet: boolean;
  is_up: boolean;
}