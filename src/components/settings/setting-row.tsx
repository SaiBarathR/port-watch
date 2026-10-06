import { useId, type ReactNode } from "react";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";

interface SettingRowProps {
  label: string;
  description?: ReactNode;
  /**
   * The control, given two ids. A switch, a select or an input takes
   * `control`: clicking the label then reaches it, and a screen reader names
   * it by the label. A button keeps its own name, which says what it does,
   * and points at `label` as its description instead.
   */
  children: (ids: { control: string; label: string }) => ReactNode;
  /** Puts the control under the text instead of beside it. */
  stacked?: boolean;
}

export function SettingRow({
  label,
  description,
  children,
  stacked = false,
}: SettingRowProps) {
  const control = useId();
  const labelId = useId();

  return (
    <div
      className={cn(
        "gap-3 py-3",
        stacked
          ? "flex flex-col"
          : "grid grid-cols-1 items-center sm:grid-cols-[minmax(0,1fr)_auto]",
      )}
    >
      <div className="min-w-0 space-y-0.5">
        <Label id={labelId} htmlFor={control} className="text-sm font-medium">
          {label}
        </Label>
        {description && (
          <p className="text-xs text-muted-foreground">{description}</p>
        )}
      </div>
      <div className={cn("min-w-0", !stacked && "sm:justify-self-end")}>
        {children({ control, label: labelId })}
      </div>
    </div>
  );
}

export function SettingSection({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <section className="min-w-0">
      <h3 className="mb-1.5 text-xs font-semibold tracking-wide text-muted-foreground uppercase">
        {title}
      </h3>
      <div className="divide-y overflow-hidden rounded-lg border bg-muted/20 px-4">
        {children}
      </div>
    </section>
  );
}
