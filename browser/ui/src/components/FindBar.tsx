import React, {useEffect, useRef} from "react";
import KeyboardArrowUpIcon from "@mui/icons-material/KeyboardArrowUp";
import KeyboardArrowDownIcon from "@mui/icons-material/KeyboardArrowDown";
import CloseIcon from "@mui/icons-material/Close";

interface FindBarProps {
    query: string;
    current: number;
    total: number;
    focusToken: number;
    onQueryChange: (query: string) => void;
    onNext: () => void;
    onPrev: () => void;
    onClose: () => void;
}

export const FindBar: React.FC<FindBarProps> = ({
    query,
    current,
    total,
    focusToken,
    onQueryChange,
    onNext,
    onPrev,
    onClose
}) => {
    const inputRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
        inputRef.current?.focus();
        inputRef.current?.select();
    }, [focusToken]);

    const noMatch = query !== "" && total === 0;
    return (
        <div className="find-bar">
            <input
                ref={inputRef}
                className={`find-input ${noMatch ? "no-match" : ""}`}
                spellCheck={false}
                value={query}
                placeholder="Find in page"
                onChange={(e) => onQueryChange(e.target.value)}
                onKeyDown={(e) => {
                    if (e.key === "Enter") {
                        e.preventDefault();
                        if (e.shiftKey) onPrev(); else onNext();
                    } else if (e.key === "Escape") {
                        e.preventDefault();
                        onClose();
                    }
                }}
            />
            <span className={`find-count ${noMatch ? "no-match" : ""}`}>
                {query ? `${current}/${total}` : ""}
            </span>
            <div className="find-sep" />
            <button className="find-btn" onClick={onPrev} disabled={total === 0} title="Previous (Shift+Enter)">
                <KeyboardArrowUpIcon sx={{fontSize: 18}} />
            </button>
            <button className="find-btn" onClick={onNext} disabled={total === 0} title="Next (Enter)">
                <KeyboardArrowDownIcon sx={{fontSize: 18}} />
            </button>
            <button className="find-btn" onClick={onClose} title="Close (Esc)">
                <CloseIcon sx={{fontSize: 16}} />
            </button>
        </div>
    );
};