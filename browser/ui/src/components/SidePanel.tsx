import React, {useState, useEffect} from "react";
import WebAssetIcon from "@mui/icons-material/WebAsset";
import OpenInNewIcon from "@mui/icons-material/OpenInNew";
import HistoryIcon from "@mui/icons-material/History";
import BookmarksOutlinedIcon from "@mui/icons-material/BookmarksOutlined";
import FileDownloadOutlinedIcon from "@mui/icons-material/FileDownloadOutlined";
import PrintOutlinedIcon from "@mui/icons-material/PrintOutlined";
import SaveAltIcon from "@mui/icons-material/SaveAlt";
import FindInPageOutlinedIcon from "@mui/icons-material/FindInPageOutlined";
import SettingsOutlinedIcon from "@mui/icons-material/SettingsOutlined";
import CloseIcon from "@mui/icons-material/Close";
import ChevronRightIcon from "@mui/icons-material/ChevronRight";
import ArrowBackIcon from "@mui/icons-material/ArrowBack";
import RemoveIcon from "@mui/icons-material/Remove";
import AddIcon from "@mui/icons-material/Add";
import OpenInFullIcon from "@mui/icons-material/OpenInFull";
import DeleteSweepIcon from "@mui/icons-material/DeleteSweep";
import SaveIcon from "@mui/icons-material/Save";
import FolderOpenOutlinedIcon from "@mui/icons-material/FolderOpenOutlined";
import ZoomInOutlinedIcon from "@mui/icons-material/ZoomInOutlined";
import {BookmarkItem, BrowserSettings, DownloadItem, HistoryEntry, PanelMode, SaveResult} from "../types";

interface SidePanelProps {
    mode: PanelMode;
    setMode: (mode: PanelMode) => void;
    historyEntries: HistoryEntry[];
    bookmarks: BookmarkItem[];
    downloads: DownloadItem[];
    settings: BrowserSettings;
    saveResult: SaveResult | null;
    zoom: number;
    onZoomChange: (nextZoom: number) => void;
    onFullscreen: () => void;
    onNewTab: () => void;
    onNewWindow: () => void;
    onPrint: () => void;
    onSavePage: () => void;
    onFind: () => void;
    onExit: () => void;
    onSaveSettings: (next: BrowserSettings) => void;
    onNavigate: (url: string) => void;
    onClearHistory: () => void;
    onOpenDownload: (path: string) => void;
    onShowDownload: (path: string) => void;
    onClearDownloads: () => void;
    onClose: () => void;
}

const downloadLabel = (d: DownloadItem): string => 
    d.state === "in_progress" ? "Downloading" : d.state === "completed" ? "Done" : "Failed";

export const SidePanel: React.FC<SidePanelProps> = ({
    mode, 
    setMode,
    historyEntries,
    bookmarks, 
    downloads,
    settings,
    saveResult,
    zoom,
    onZoomChange,
    onFullscreen,
    onNewTab,
    onNewWindow,
    onPrint,
    onSavePage,
    onFind,
    onExit,
    onSaveSettings,
    onNavigate,
    onClearHistory,
    onOpenDownload,
    onShowDownload,
    onClearDownloads,
    onClose
}) => {
    const [draft, setDraft] = useState<BrowserSettings>(settings);
    
    useEffect(() => {
        setDraft(settings);
    }, [settings]);

    if (mode === "none") return null;
    
    return (
        <div className="chrome-menu-backdrop" onClick={onClose}>
            <div className="chrome-menu-popover" onClick={(e) => e.stopPropagation()}>
                {mode === "menu" && (
                    <>
                        <button className="cm-item cm-profile" onClick={() => setMode("settings")}>
                            <span className="cm-profile-name">sonion</span>
                            <ChevronRightIcon sx={{fontSize: 18, color: "#9aa0a6"}} />
                        </button>
                        <div className="cm-divider" /> 
                        <button className="cm-item" onClick={() => {
                            onNewTab();
                            onClose();
                        }}>
                            <div className="cm-left">
                                <WebAssetIcon sx={{fontSize: 18}} />
                                <span>New Tab</span>
                            </div>
                        </button>
                        <button className="cm-item" onClick={() => {
                            onNewWindow();
                            onClose();
                        }}>
                            <div className="cm-left">
                                <OpenInNewIcon sx={{fontSize: 18}} />
                                <span>New Window</span>
                            </div>
                        </button>
                        <div className="cm-divider" />
                        <button className="cm-item" onClick={() => setMode("history")}>
                            <div className="cm-left">
                                <HistoryIcon sx={{ fontSize: 18 }} />
                                <span>History</span>
                            </div>
                            <ChevronRightIcon sx={{ fontSize: 18, color: "#9aa0a6" }} />
                        </button>
                        <button className="cm-item" onClick={() => setMode("bookmarks")}>
                            <div className="cm-left">
                                <BookmarksOutlinedIcon sx={{ fontSize: 18 }} />
                                <span>Bookmarks</span>
                            </div>
                            <ChevronRightIcon sx={{ fontSize: 18, color: "#9aa0a6" }} />
                        </button>
                        <button className="cm-item" onClick={() => setMode("downloads")}>
                            <div className="cm-left">
                                <FileDownloadOutlinedIcon sx={{ fontSize: 18 }} />
                                <span>Downloads</span>
                            </div>
                            <ChevronRightIcon sx={{ fontSize: 18, color: "#9aa0a6" }} />
                        </button>
                        <div className="cm-divider" />
                        <button className="cm-item" onClick={() => {
                                onPrint();
                                onClose();
                            }}>
                            <div className="cm-left">
                                <PrintOutlinedIcon sx={{ fontSize: 18 }} />
                                <span>Print</span>
                            </div>
                        </button>
                        <button className="cm-item" onClick={() => {
                                onSavePage();
                                onClose();
                            }}>
                            <div className="cm-left">
                                <SaveAltIcon sx={{ fontSize: 18 }} />
                                <span>Save Page As</span>
                            </div>
                        </button>
                        <button className="cm-item" onClick={onFind}>
                            <div className="cm-left">
                                <FindInPageOutlinedIcon sx={{ fontSize: 18 }} />
                                <span>Find</span>
                            </div>
                        </button>
                        <div className="cm-divider" />
                        <div className="cm-zoom-row">
                            <div className="cm-left">
                                <ZoomInOutlinedIcon sx={{ fontSize: 18 }} />
                                <span>Zoom</span>
                            </div>
                            <div className="cm-zoom-controls">
                                <button className="cm-zoom-btn"
                                    onClick={() => onZoomChange(Math.max(50, zoom - 10))}
                                    title="Zoom out">
                                    <RemoveIcon sx={{ fontSize: 16 }} />
                                </button>
                                <span className="cm-zoom-val">{zoom}%</span>
                                <button className="cm-zoom-btn"
                                    onClick={() => onZoomChange(Math.min(200, zoom + 10))}
                                    title="Zoom in">
                                    <AddIcon sx={{ fontSize: 16 }} />
                                </button>
                                <div className="cm-zoom-sep" />
                                <button className="cm-zoom-btn"
                                    onClick={() => {
                                        onFullscreen();
                                        onClose();
                                    }}
                                    title="Fullscreen">
                                    <OpenInFullIcon sx={{ fontSize: 15 }} />
                                </button>
                            </div>
                        </div>
                        <button className="cm-item" onClick={() => setMode("settings")}>
                            <div className="cm-left">
                                <SettingsOutlinedIcon sx={{ fontSize: 18 }} />
                                <span>Settings</span>
                            </div>
                        </button>
                        <div className="cm-divider" />
                        <button className="cm-item" onClick={onExit}>
                            <div className="cm-left">
                                <CloseIcon sx={{ fontSize: 18 }} />
                                <span>Exit</span>
                            </div>
                        </button>
                    </>
                )}
                {mode === "history" && (
                    <div className="cm-subview">
                        <div className="cm-subheader">
                            <button className="cm-zoom-btn" onClick={() => setMode("menu")}>
                                <ArrowBackIcon sx={{ fontSize: 16 }} />
                            </button>
                            <span>History ({historyEntries.length})</span>
                            <button className="cm-zoom-btn" onClick={onClearHistory} title="Clear History">
                                <DeleteSweepIcon sx={{ fontSize: 16 }} />
                            </button>
                        </div>
                        <div className="cm-sublist">
                            {historyEntries.length === 0 ? (
                                <div className="cm-empty">No history entries yet.</div>
                            ) : (
                                historyEntries.map((item) => (
                                    <button key={item.id} className="cm-item" onClick={() => {
                                            onNavigate(item.url);
                                            onClose();
                                        }}>
                                        <span className="cm-url">{item.url}</span>
                                        <span className="cm-shortcut">
                                            {new Date(item.timestamp).toLocaleTimeString([], {
                                                hour: "2-digit",
                                                minute: "2-digit",
                                            })}
                                        </span>
                                    </button>
                                ))
                            )}
                        </div>
                    </div>
                )}
                {mode === "bookmarks" && (
                    <div className="cm-subview">
                        <div className="cm-subheader">
                            <button className="cm-zoom-btn" onClick={() => setMode("menu")}>
                                <ArrowBackIcon sx={{ fontSize: 16 }} />
                            </button>
                            <span>Bookmarks ({bookmarks.length})</span>
                            <span />
                        </div>
                        <div className="cm-sublist">
                            {bookmarks.map((bm) => (
                                <button key={bm.id} className="cm-item" onClick={() => {
                                        onNavigate(bm.url);
                                        onClose();
                                    }}>
                                    <span>{bm.title}</span>
                                    <span className="cm-shortcut">{bm.url}</span>
                                </button>
                            ))}
                        </div>
                    </div>
                )}
                {mode === "downloads" && (
                    <div className="cm-subview">
                        <div className="cm-subheader">
                            <button className="cm-zoom-btn" onClick={() => setMode("menu")}>
                                <ArrowBackIcon sx={{ fontSize: 16 }} />
                            </button>
                            <span>Downloads ({downloads.length})</span>
                            <button className="cm-zoom-btn" onClick={onClearDownloads} title="Clear finished">
                                <DeleteSweepIcon sx={{ fontSize: 16 }} />
                            </button>
                        </div>
                        <div className="cm-sublist">
                            {downloads.length === 0 ? (
                                <div className="cm-empty">No downloads yet.</div>
                            ) : (
                                downloads.map((d) => (
                                    <div key={d.id} className="cm-download">
                                        <div className="cm-download-info">
                                            <span className="cm-download-name" title={d.path}>{d.fileName}</span>
                                            <span className={`cm-download-state ${d.state}`}>{downloadLabel(d)}</span>
                                        </div>
                                        <button className="cm-zoom-btn" disabled={d.state !== "completed"}
                                            onClick={() => onOpenDownload(d.path)} title="Open">
                                            <OpenInNewIcon sx={{ fontSize: 16 }} />
                                        </button>
                                        <button className="cm-zoom-btn" disabled={d.state !== "completed"}
                                            onClick={() => onShowDownload(d.path)} title="Show in folder">
                                            <FolderOpenOutlinedIcon sx={{ fontSize: 16 }} />
                                        </button>
                                    </div>
                                ))
                            )}
                        </div>
                    </div>
                )}
                {mode === "settings" && (
                    <div className="cm-subview">
                        <div className="cm-subheader">
                            <button className="cm-zoom-btn" onClick={() => setMode("menu")}>
                                <ArrowBackIcon sx={{ fontSize: 16 }} />
                            </button>
                            <span>Settings</span>
                            <span />
                        </div>
                        <div className="cm-settings-body">
                            <label className="cm-field">
                                <span>Registry Server</span>
                                <input type="text" spellCheck={false} value={draft.registry} onChange={(e) =>
                                        setDraft({ ...draft, registry: e.target.value })
                                    } />
                            </label>
                            <label className="cm-field">
                                <span>Homepage</span>
                                <input type="text" spellCheck={false} value={draft.homepage} onChange={(e) =>
                                        setDraft({ ...draft, homepage: e.target.value })
                                    } />
                            </label>
                            <label className="cm-field">
                                <span>Pin Key (Ed25519 Base64)</span>
                                <input type="text" spellCheck={false} value={draft.pin_key} placeholder="Optional" onChange={(e) =>
                                        setDraft({ ...draft, pin_key: e.target.value })
                                    } />
                            </label>
                            <label className="cm-check">
                                <input type="checkbox" checked={draft.insecure_dev} onChange={(e) =>
                                        setDraft({ ...draft, insecure_dev: e.target.checked })
                                    } />
                                <span>Allow self-signed certs (insecure_dev)</span>
                            </label>
                            <button className="cm-save-btn" onClick={() => onSaveSettings(draft)}>
                                <SaveIcon sx={{ fontSize: 15 }} />
                                <span>{saveResult?.ok ? "Saved!" : "Save Settings"}</span>
                            </button>
                            {saveResult && !saveResult.ok && (
                                <div className="cm-error">{saveResult.error}</div>
                            )}
                        </div>
                    </div>
                )}
            </div>
        </div>
    );
};