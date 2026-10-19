import { useEffect, useRef, useState } from "react";

interface MenuItem {
  label: string;
  onClick: () => void;
  disabled?: boolean;
}

export function OverflowMenu({ items }: { items: MenuItem[] }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!open) return;
    function onClickOutside(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    window.addEventListener("mousedown", onClickOutside);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onClickOutside);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div className="overflow-menu" ref={ref}>
      <button className="ghost-button overflow-trigger" onClick={() => setOpen((o) => !o)} aria-label="More actions">
        ⋯
      </button>
      {open && (
        <div className="overflow-dropdown">
          {items.map((item) => (
            <button
              key={item.label}
              className="overflow-item"
              disabled={item.disabled}
              onClick={() => {
                item.onClick();
                setOpen(false);
              }}
            >
              {item.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
