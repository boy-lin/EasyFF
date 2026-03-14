import { memo, useEffect, useMemo, useRef, useState } from "react";
import { FixedSizeList, type ListChildComponentProps, type ListOnItemsRenderedProps } from "react-window";
import { Button } from "@/components/ui/button";

type VirtualLogViewerProps = {
  lines: string[];
  height?: number;
  rowHeight?: number;
  emptyText?: string;
  backToBottomText?: string;
};

type RowData = {
  lines: string[];
  emptyText: string;
};

const LogRow = memo(function LogRow({
  index,
  style,
  data,
}: ListChildComponentProps<RowData>) {
  if (data.lines.length === 0) {
    return (
      <div style={style} className="px-2 text-muted-foreground">
        {data.emptyText}
      </div>
    );
  }

  return (
    <div style={style} className="px-2">
      <div className="truncate">{data.lines[index]}</div>
    </div>
  );
});

export function VirtualLogViewer({
  lines,
  height = 192,
  rowHeight = 22,
  emptyText = "No logs yet",
  backToBottomText = "Back to bottom",
}: VirtualLogViewerProps) {
  const [followTail, setFollowTail] = useState(true);
  const listRef = useRef<FixedSizeList<RowData> | null>(null);

  const rowCount = lines.length;
  const safeCount = Math.max(rowCount, 1);
  const itemData = useMemo<RowData>(() => ({ lines, emptyText }), [lines, emptyText]);

  useEffect(() => {
    if (rowCount === 0) {
      setFollowTail(true);
      return;
    }
    if (!followTail) return;
    const id = window.requestAnimationFrame(() => {
      listRef.current?.scrollToItem(rowCount - 1, "end");
    });
    return () => window.cancelAnimationFrame(id);
  }, [followTail, rowCount]);

  const handleItemsRendered = ({ visibleStopIndex }: ListOnItemsRenderedProps) => {
    const nearBottom = rowCount === 0 || visibleStopIndex >= rowCount - 2;
    if (nearBottom !== followTail) {
      setFollowTail(nearBottom);
    }
  };

  return (
      <div className="relative rounded-md border bg-muted/30 p-1 font-mono text-xs">
        {rowCount > 0 && !followTail && (
          <div className="absolute top-2 right-4 z-10">
            <Button
              className="text-xs"
              type="button"
              size="sm"
              variant="secondary"
              onClick={() => setFollowTail(true)}
            >
              {backToBottomText}
            </Button>
          </div>
        )}
        <FixedSizeList
          ref={listRef}
          width="100%"
          height={height}
          itemCount={safeCount}
          itemSize={rowHeight}
          itemData={itemData}
          onItemsRendered={handleItemsRendered}
          overscanCount={12}
        >
          {LogRow}
        </FixedSizeList>
      </div>
  );
}
