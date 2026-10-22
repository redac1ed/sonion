import React from "react";
import ArrowBackIcon from "@mui/icons-material/ArrowBack";
import ArrowForwardIcon from "@mui/icons-material/ArrowForward";
import RefreshIcon from "@mui/icons-material/Refresh";
import HomeIcon from "@mui/icons-material/Home";
import LockIcon from "@mui/icons-material/Lock";
import PublicIcon from "@mui/icons-material/Public";
import StarBorderIcon from "@mui/icons-material/StarBorder";
import StarIcon from "@mui/icons-material/Star"
import HistoryIcon from "@mui/icons-material/History";
import TerminalIcon from "@mui/icons-material/Terminal";
import MoreVertIcon from "@mui/icons-material/MoreVert";
import {PanelMode, TabItem} from "../types"

interface ToolbarProps {
    activeTab: TabItem;
    addressInput: string;
    isBookmarked: boolean;
    panelMode: PanelMode;
    onAddressChange: (value: string) => void;
    onNavigateSubmit: () => void;
    onBack: () => void;
    onForward: () => void;
    onReload: () => void;
    onHome: () => void;
    onToggleBookmark: () => void;
    onTogglePanel: (mode: PanelMode) => void;
    onOpenDevtools: () => void;
}

export const Toolbar: React.FC<ToolbarProps> = ({
    activeTab, 
    addressInput, 
    isBookmarked,
    panelMode,
    onAddressChange,
    onNavigateSubmit,
    onBack,
    onForward,
    onReload, 
    onHome, 
    onToggleBookmark,
    onTogglePanel, 
    onOpenDevtools
}) => {
    const canGoBack = activeTab.historyIndex > 0;
    const canGoForward = activeTab.historyIndex < activeTab.history.length - 1;
    const isSonion = activeTab.url.startsWith("sonion://");
    return (
        <div className="toolbar">
            <button
                className="icon-btn"
                disabled={!canGoBack}
                onClick={onBack}
                title="Back (Alt+Left)"
            >
                <ArrowBackIcon sx={{fontSize: 18}} />
            </button>
            <button
                className="icon-btn"
                disabled={!canGoForward}
                onClick={onForward}
                title="Forward (Alt+Right)"
            >
                <ArrowForwardIcon sx={{fontSize: 18}} /> 
            </button>
            <button 
                className="icon-btn"
                onClick={onReload}
                title="Reload (Ctrl+R)"
            >
                <RefreshIcon sx={{fontSize: 18}} /> 
            </button>
            <button 
                className="icon-btn"
                onClick={onHome}
                title="Home"
            >
                <HomeIcon sx={{fontSize: 18}} />
            </button>
            <div className="omnibox">
                <span
                    className="omnibox-security"
                    title={isSonion ? "TLS 1.3 + Ed25519 Signed Resolution" : "Internal Page"}
                >
                    {isSonion ? (
                        <LockIcon sx={{fontSize: 15}} /> 
                    ) : (
                        <PublicIcon sx={{fontSize: 15, color: "#949ba4"}} />
                    )}
                </span>
                <input 
                    className="omnibox-input"
                    spellCheck={false}
                    value={addressInput}
                    placeholder="Search or enter sonion:// address"
                    onChange={(e) => onAddressChange(e.target.value)}
                    onKeyDown={(e) => {
                        if (e.key === "Enter") onNavigateSubmit();
                    }}
                    onFocus={(e) => e.currentTarget.select()}
                />
                <button 
                    className="icon-btn"
                    style={{width: 26, height: 26}}
                    onClick={onToggleBookmark}
                    title={isBookmarked ? "Remove bookmark" : "Bookmark this page"}
                >
                    {isBookmarked ? (
                        <StarIcon sx={{fontSize: 17, color: "#facc15"}} />
                    ) : (
                        <StarBorderIcon sx={{fontSize:17}} />
                    )}
                </button>
            </div>
            <button 
                className={`icon-btn ${panelMode === "history" ? "active-toggle" : ""}`}
                onClick={() => 
                    onTogglePanel(panelMode === "history" ? "none" : "history")
                }
                title="History"
            >
                <HistoryIcon sx={{fontSize: 18}} />
            </button>
            <button 
                className="icon-btn"
                onClick={onOpenDevtools}
                title="Developer Tools"
            >
                <TerminalIcon sx={{fontSize: 18}} /> 
            </button>
            <button
                className={`icon-btn ${panelMode === "settings" ? "active-toggle" : ""}`}
                onClick={() => 
                    onTogglePanel(panelMode === "settings" ? "none": "settings")
                }
                title="Browser Info & Settings"
            >
                {/* <MoreVertIcon sx={{fontSize:18}} /> */}
            </button>
        </div>
    );
};