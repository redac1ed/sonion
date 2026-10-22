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

export type PanelMode = "none" | "history" | "bookmarks" | "settings";

export type IpcCommand = 
    | {cmd: "create_tab"; tabId: string; url: string}
    | {cmd: "switch_tab"; tabId: string}
    | {cmd: "close_tab"; tabId: string}
    | {cmd: "nav"; tabId: string; url: string}
    | {cmd: "reload"; tabId: string}
    | {cmd: "devtools"; tabId: string}
    | {cmd: "set_chrome_height"; height: number}
    | {cmd: "external"; url: string}

export interface NativeHostEvent {
    event: "page_loaded" | "title_changed" | "url_changed";
    tabId: string;
    url?: string;
    title?: string;
}

