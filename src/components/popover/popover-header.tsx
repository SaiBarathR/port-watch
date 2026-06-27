import { useRef } from "react";
import { SearchIcon, XIcon } from "lucide-react";
import { Input } from "@/components/ui/input";

interface PopoverHeaderProps {
  search: string;
  onSearchChange: (value: string) => void;
}

export function PopoverHeader({ search, onSearchChange }: PopoverHeaderProps) {
  const inputRef = useRef<HTMLInputElement>(null);

  return (
    <div className="[-webkit-app-region:drag] shrink-0 border-b px-3 pt-3 pb-2">
      <div className="[-webkit-app-region:no-drag] relative">
        <SearchIcon
          className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
          aria-hidden
        />
        <Input
          ref={inputRef}
          className="h-8 bg-muted/30 pl-8 pr-8 text-sm"
          placeholder="Search port or process…"
          inputMode="search"
          value={search}
          onChange={(event) => onSearchChange(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              if (search) {
                onSearchChange("");
                event.stopPropagation();
              }
            }
          }}
        />
        {search && (
          <button
            type="button"
            className="absolute top-1/2 right-2 -translate-y-1/2 rounded p-0.5 text-muted-foreground hover:text-foreground"
            onClick={() => {
              onSearchChange("");
              inputRef.current?.focus();
            }}
            aria-label="Clear search"
          >
            <XIcon className="size-3.5" />
          </button>
        )}
      </div>
    </div>
  );
}
