declare module "react-window" {
  import * as React from "react";

  export type Align = "auto" | "smart" | "center" | "end" | "start";

  export interface ListOnItemsRenderedProps {
    overscanStartIndex: number;
    overscanStopIndex: number;
    visibleStartIndex: number;
    visibleStopIndex: number;
  }

  export interface ListChildComponentProps<T = any> {
    index: number;
    style: React.CSSProperties;
    data: T;
    isScrolling?: boolean;
  }

  export interface FixedSizeListProps<T = any> {
    children: React.ComponentType<ListChildComponentProps<T>>;
    height: number;
    width: number | string;
    itemCount: number;
    itemSize: number;
    itemData?: T;
    overscanCount?: number;
    onItemsRendered?: (props: ListOnItemsRenderedProps) => void;
  }

  export class FixedSizeList<T = any> extends React.PureComponent<FixedSizeListProps<T>> {
    scrollTo(scrollOffset: number): void;
    scrollToItem(index: number, align?: Align): void;
  }
}
