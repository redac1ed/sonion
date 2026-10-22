import { IpcCommand, NativeHostEvent } from "./types";

declare global {
    interface Window {
        ipc?: {
            postMessage: (msg: string) => void;
        };
        _onNativeEvent?: (ev: NativeHostEvent) => void;
    }
}

export function sendIpc(command: IpcCommand): void {
    if (window.ipc) {
        window.ipc.postMessage(JSON.stringify(command));
    } else {
        console.debug("[IPC Mock]", command);
    }
}

export function normalizeSonionUrl(raw: string): string | null {
     const input = raw.trim();
     if (!input) return null;
     if (
        input.startsWith("sonion://") ||
        input.startsWith("sonion-nav://") ||
        input.startsWith("http://") ||
        input.startsWith("https://")
     ) {
        return input;
     }
     return `sonion://${input}`
}

export function extractDisplayTitle(url: string): string {
    if (url.startsWith("sonion-nav://home")) return "New Tab";
    if (url.startsWith("sonion-nav://history")) return "History";
    if (url.startsWith("sonion-nav://settings")) return "Settings";
    try {
        const cleaned = url.replace(/^sonion:\/\//, "http://");
        const parsed = new URL(cleaned);
        return parsed.host + (parsed.pathname !== "/" ? parsed.pathname : "");
    } catch {
        return url;
    }
}