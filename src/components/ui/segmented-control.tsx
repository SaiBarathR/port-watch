import { useRef, type ReactNode } from "react";
import { cn } from "@/lib/utils";

interface SegmentedControlProps<Value extends string> {
  value: Value;
  onValueChange: (value: Value) => void;
  options: { value: Value; label: ReactNode }[];
  "aria-label": string;
  className?: string;
}

/** A choice of one among a few, all in view: a radio group drawn as one bar. */
export function SegmentedControl<Value extends string>({
  value,
  onValueChange,
  options,
  className,
  ...props
}: SegmentedControlProps<Value>) {
  const groupRef = useRef<HTMLDivElement>(null);

  // One tab stop for the group; the arrow keys move within it and choose.
  const onKeyDown = (event: React.KeyboardEvent) => {
    const step =
      event.key === "ArrowRight" || event.key === "ArrowDown"
        ? 1
        : event.key === "ArrowLeft" || event.key === "ArrowUp"
          ? -1
          : 0;
    if (step === 0) {
      return;
    }
    event.preventDefault();
    const index = options.findIndex((option) => option.value === value);
    const next = options[(index + step + options.length) % options.length];
    onValueChange(next.value);
    groupRef.current
      ?.querySelector<HTMLElement>(`[data-value="${next.value}"]`)
      ?.focus();
  };

  return (
    <div
      ref={groupRef}
      role="radiogroup"
      aria-label={props["aria-label"]}
      className={cn(
        "inline-flex items-center rounded-lg border bg-muted/40 p-0.5",
        className,
      )}
      onKeyDown={onKeyDown}
    >
      {options.map((option) => {
        const selected = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={selected}
            data-value={option.value}
            tabIndex={selected ? 0 : -1}
            className={cn(
              "inline-flex h-7 items-center gap-1.5 rounded-md px-2.5 text-sm font-medium whitespace-nowrap text-muted-foreground outline-none transition-colors hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring",
              selected && "bg-background text-foreground shadow-xs",
            )}
            onClick={() => onValueChange(option.value)}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
