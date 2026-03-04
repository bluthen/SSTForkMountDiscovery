export interface DeviceInfo {
  hostname: string;
  ip: string;
  deviceType: "sst" | "allsky";
}

export interface ScanProgress {
  percentage: number;
  message: string;
}
