export interface TabItem {
    id: string;
    title: string;
    url: string;
    history: string[];
    historyIndex: number;
    loading: boolean;
    secure: boolean;
}

export interface BookmarkItem {
    id: string;
    title: string;
    url: string;
}

export interface HistoryEntry {
    id: string;
    title: string;
    url: string;
    timestamp: number;
}

export interface DownloadItem {
    id: number;
    url: string;
    path: string;
    fileName: string;
    state: "in_progress" | "completed" | "failed";
}

export type PanelMode = "none"| "menu" | "history" | "bookmarks" | "downloads" | "settings";

export type WindowCommand = "win_minimize" | "win_toggle_maximize" | "win_drag" | "win_resize_top" | "exit";

export type IpcCommand = 
    | {cmd: "create_tab"; tabId: string; url: string}
    | {cmd: "switch_tab"; tabId: string}
    | {cmd: "close_tab"; tabId: string}
    | {cmd: "nav"; tabId: string; url: string}
    | {cmd: "reload"; tabId: string}
    | {cmd: "devtools"; tabId: string}
    | {cmd: "set_chrome_height"; height: number; overlay: boolean}
    | {cmd: "external"; url: string}
    | {cmd: "get_settings"}
    | {cmd: "save_settings"; settings: BrowserSettings}
    | {cmd: "zoom"; tabId: string; zoom: number}
    | {cmd: "print"; tabId: string;}
    | {cmd: "fullscreen"}
    | {cmd: "exit"}
    | {cmd: WindowCommand}
    | {cmd: "new_window"}
    | {cmd: "save_page"; tabId: string}
    | {cmd: "find"; tabId: string; query: string; dir: number}
    | {cmd: "find_clear"; tabId: string}
    | {cmd: "open_download"; path: string}
    | {cmd: "show_download"; path: string}
    | {cmd: "save_profile"; profile: Profile}

export interface NativeHostEvent {
    event: "page_loaded" | "title_changed" | "url_changed" | "page_started" | "window_state" | "settings_saved" | "download" | "find_result" | "shortcut" | "open";
    tabId: string;
    url?: string;
    title?: string;
    settings?: BrowserSettings;
    maximized?: boolean;
    ok?: boolean;
    error?: string;
    download?: DownloadItem;
    current?: number;
    total?: number;
    key?: string;
}

export interface BrowserSettings {
    registry: string;
    homepage: string;
    pin_key: string;
    insecure_dev: boolean;
}

export interface SaveResult {
    ok: boolean;
    error?: string;
}

export interface Profile {
    bookmarks: BookmarkItem[];
    history: HistoryEntry[];
}