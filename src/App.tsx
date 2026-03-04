import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  ThemeProvider,
  createTheme,
  CssBaseline,
  Box,
  Typography,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Paper,
  Button,
  TextField,
  LinearProgress,
  Alert,
  Stack,
  Chip,
} from "@mui/material";
import SearchIcon from "@mui/icons-material/Search";
import LinkIcon from "@mui/icons-material/Link";
import WifiIcon from "@mui/icons-material/Wifi";
import type { DeviceInfo, ScanProgress } from "./types";

const darkTheme = createTheme({
  palette: {
    mode: "dark",
  },
});

const IP_REGEX =
  /^(?!\.)((\.|^)([1-9]?\d|1\d\d|2(5[0-5]|[0-4]\d))){4}$/;

function App() {
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [selectedIp, setSelectedIp] = useState<string | null>(null);
  const [manualIp, setManualIp] = useState("");
  const [scanProgress, setScanProgress] = useState<ScanProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [connecting, setConnecting] = useState(false);

  const isManualIpValid = IP_REGEX.test(manualIp);

  // Listen for device updates from the backend
  useEffect(() => {
    const unlistenDevices = listen<DeviceInfo[]>("devices-updated", (event) => {
      setDevices(event.payload);
    });

    const unlistenProgress = listen<ScanProgress>("scan-progress", (event) => {
      setScanProgress(event.payload);
      if (event.payload.percentage >= 100) {
        // Clear progress after a short delay
        setTimeout(() => setScanProgress(null), 2000);
      }
    });

    return () => {
      unlistenDevices.then((fn) => fn());
      unlistenProgress.then((fn) => fn());
    };
  }, []);

  // Start discovery on mount
  useEffect(() => {
    invoke("start_discovery").catch((err) => {
      console.error("Failed to start discovery:", err);
    });
  }, []);

  // Auto-select if only one device
  useEffect(() => {
    if (devices.length === 1 && !selectedIp) {
      setSelectedIp(devices[0].ip);
    }
  }, [devices, selectedIp]);

  const handleRowClick = useCallback((ip: string) => {
    setSelectedIp(ip);
    setError(null);
  }, []);

  const handleConnect = useCallback(async () => {
    if (!selectedIp) return;
    setConnecting(true);
    setError(null);
    try {
      await invoke("connect_to_device", { ip: selectedIp });
    } catch (err) {
      setError(String(err));
    } finally {
      setConnecting(false);
    }
  }, [selectedIp]);

  const handleManualConnect = useCallback(async () => {
    if (!isManualIpValid) return;
    setConnecting(true);
    setError(null);
    try {
      await invoke("check_ip", { ip: manualIp });
      await invoke("connect_to_device", { ip: manualIp });
    } catch (err) {
      setError(`Unable to connect to ${manualIp}`);
    } finally {
      setConnecting(false);
    }
  }, [manualIp, isManualIpValid]);

  const handleManualKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && isManualIpValid) {
        handleManualConnect();
      }
    },
    [handleManualConnect, isManualIpValid]
  );

  return (
    <ThemeProvider theme={darkTheme}>
      <CssBaseline />
      <Box sx={{ p: 2, height: "100vh", display: "flex", flexDirection: "column" }}>
        <Typography variant="h4" gutterBottom sx={{ display: "flex", alignItems: "center", gap: 1 }}>
          <WifiIcon /> SSTEQ25 Discover
        </Typography>

        {/* Device Table */}
        <TableContainer
          component={Paper}
          variant="outlined"
          sx={{ flex: 1, minHeight: 0, mb: 2 }}
        >
          <Table stickyHeader size="small">
            <TableHead>
              <TableRow>
                <TableCell>Hostname</TableCell>
                <TableCell>IP Address</TableCell>
                <TableCell>Type</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {devices.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={3} align="center" sx={{ py: 4, color: "text.secondary" }}>
                    <SearchIcon sx={{ fontSize: 40, mb: 1, display: "block", mx: "auto", opacity: 0.5 }} />
                    Searching for devices...
                  </TableCell>
                </TableRow>
              ) : (
                devices.map((device) => (
                  <TableRow
                    key={device.ip}
                    hover
                    selected={selectedIp === device.ip}
                    onClick={() => handleRowClick(device.ip)}
                    sx={{ cursor: "pointer" }}
                  >
                    <TableCell>{device.hostname}</TableCell>
                    <TableCell>{device.ip}</TableCell>
                    <TableCell>
                      <Chip
                        label={device.deviceType === "sst" ? "SST Mount" : "AllSky"}
                        color={device.deviceType === "sst" ? "primary" : "secondary"}
                        size="small"
                        variant="outlined"
                      />
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </TableContainer>

        {/* Scan Progress */}
        {scanProgress && (
          <Box sx={{ mb: 2 }}>
            <Typography variant="body2" color="text.secondary" sx={{ mb: 0.5 }}>
              {scanProgress.message}
            </Typography>
            <LinearProgress
              variant="determinate"
              value={scanProgress.percentage}
            />
          </Box>
        )}

        {/* Error Alert */}
        {error && (
          <Alert severity="error" sx={{ mb: 2 }} onClose={() => setError(null)}>
            {error}
          </Alert>
        )}

        {/* Controls */}
        <Stack direction="row" spacing={2} alignItems="center">
          <TextField
            size="small"
            placeholder="xxx.xxx.xxx.xxx"
            label="Manual IP"
            value={manualIp}
            onChange={(e) => setManualIp(e.target.value)}
            onKeyDown={handleManualKeyDown}
            sx={{ width: 200 }}
          />
          <Button
            variant="outlined"
            color="warning"
            disabled={!isManualIpValid || connecting}
            onClick={handleManualConnect}
            startIcon={<LinkIcon />}
          >
            Manual Connect
          </Button>
          <Box sx={{ flex: 1 }} />
          <Button
            variant="contained"
            color="success"
            disabled={!selectedIp || connecting}
            onClick={handleConnect}
            startIcon={<LinkIcon />}
            size="large"
          >
            Connect
          </Button>
        </Stack>
      </Box>
    </ThemeProvider>
  );
}

export default App;
