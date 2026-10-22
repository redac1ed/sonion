import React from "react";
import CloseIcon from "@mui/icons-material/Close";
import DeleteSweepIcon from "@mui/icons-material/DeleteSweep";
import {HistoryEntry, PanelMode} from "../types";
import { Delete } from "@mui/icons-material";

interface SidePanelProps {
    mode: PanelMode;
    historyEntries: HistoryEntry[];
    onNavigate: (url: string) => void;
    onClearHistory: () => void;
    onClose: () => void;
}

export const SidePanel: React.FC<SidePanelProps> = ({
    mode, 
    historyEntries, 
    onNavigate,
    onClearHistory,
    onClose
}) => {
    if (mode === "none") return null;
    return (
        <div className="drawer-panel">
            <div className="drawer-header">
                <span> 
                    Browsing History ({historyEntries.length})
                </span>
                <div style={{display: "flex", gap: 6}}>
                    {mode === "history" && historyEntries.length > 0 && (
                        <button
                            className="icon-btn"
                            style={{width: 24, height: 24}}
                            onClick={onClearHistory}
                            title="Clear History"
                        >
                            <DeleteSweepIcon sx={{fontSize: 16}} /> 
                        </button>
                    )}
                    <button 
                        className="icon-btn"
                        style={{width: 24, height: 24}}
                        onClick={onClose}
                        title="Close Panel"
                    >
                        <CloseIcon sx={{fontSize: 16}} />
                    </button>
                </div>
            </div>
            {mode === "history" && (
                <div className="drawer-list">
                    {historyEntries.length === 0 ? (
                        <div style={{fontSize: 12, color: "#949ba4"}}>
                            No history entries yet.
                        </div>
                    ) : (
                        historyEntries.map((item) => (
                            <div
                                key={item.id}
                                className="drawer-row"
                                onClick={() => {
                                    onNavigate(item.url);
                                    onClose();
                                }}
                            > 
                                <span style={{color: "#67e8f9", fontFamily: "monospace"}}>
                                    {item.url}
                                </span>
                                <span style={{color: "#949ba4", fontSize: 11}}>
                                    {new Date(item.timestamp).toLocaleTimeString()}
                                </span>
                            </div>
                        ))
                    )}
                </div>
            )}
        </div>
    );
};