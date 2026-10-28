import React from "react";
import AddIcon from "@mui/icons-material/Add";
import CloseIcon from "@mui/icons-material/Close";
import PublicIcon from "@mui/icons-material/Public";
import AutorenewIcon from "@mui/icons-material/Autorenew";
import {TabItem, WindowCommand} from "../types";
import {WindowControls} from "./WindowControls";

interface TabStripProps {
    tabs: TabItem[];
    activeTabId: string;
    maximized: boolean;
    onSelectTab: (tabId: string) => void;
    onCloseTab: (tabId: string, e: React.MouseEvent) => void;
    onNewTab: () => void;
    onWindowCommand: (cmd: WindowCommand) => void;
}

export const TabStrip: React.FC<TabStripProps> = ({
    tabs, 
    activeTabId,
    maximized,
    onSelectTab,
    onCloseTab,
    onNewTab,
    onWindowCommand
}) => {
    const handleDragMouseDown = (e: React.MouseEvent) => {
        if (e.button !== 0) return;
        if ((e.target as HTMLElement).closest(".tab-item, button")) return;
        onWindowCommand(e.detail === 2 ? "win_toggle_maximize" : "win_drag");
    };
    return (
        <div className="tab-strip" onMouseDown={handleDragMouseDown}>
            {!maximized && (
                <div className="window-resize-top" onMouseDown={(e) => {
                    if (e.button !== 0) return;
                    e.stopPropagation();
                    onWindowCommand("win_resize_top");
                }} />
            )}
            <div className="tab-list">
                {tabs.map((tab) => {
                    const isActive = tab.id === activeTabId;
                    return (
                        <div
                            key={tab.id}
                            className={`tab-item ${isActive ? "active" : ""}`}
                            onClick={() => onSelectTab(tab.id)}
                            title={tab.url}
                        >
                            {tab.loading ? (
                            <AutorenewIcon className="spin-icon" sx={{ fontSize: 15, color: "#ffffff" }} />
                            ) : (
                                <PublicIcon
                                    sx={{
                                    fontSize: 15,
                                    color: isActive ? "#ffffff" : "#949ba4",
                                }}/>
                            )}
                            <span className="tab-title">{tab.title || "New Tab"}</span>
                            <button
                                className="tab-close-btn"
                                onClick={(e) => onCloseTab(tab.id, e)}
                                title="Close tab"
                            >
                            <CloseIcon sx={{ fontSize: 14 }} />
                            </button>
                        </div>
                    );
                })}
                <button className="new-tab-btn" onClick={onNewTab} title="New tab (Ctrl+T)">
                    <AddIcon sx={{ fontSize: 18 }} />
                </button>
            </div>
            <WindowControls maximized={maximized} onWindowCommand={onWindowCommand} />
        </div>
    );
};