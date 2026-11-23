import {
  type ColumnDef,
  type SortingState,
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { ArrowDown, ArrowUp, ArrowUpDown } from "lucide-react";
import { type ReactNode, useState } from "react";

export interface DataTableProps<TData, TValue> {
  columns: ColumnDef<TData, TValue>[];
  data: TData[];
  emptyMessage?: string;
  emptyDescription?: string;
  emptyIcon?: ReactNode;
  loading?: boolean;
  loadingRows?: number;
  sortable?: boolean;
  initialSorting?: SortingState;
}

export function DataTable<TData, TValue>({
  columns,
  data,
  emptyMessage = "No records found",
  emptyDescription,
  emptyIcon,
  loading = false,
  loadingRows = 4,
  sortable = false,
  initialSorting = [],
}: DataTableProps<TData, TValue>) {
  const [sorting, setSorting] = useState<SortingState>(initialSorting);

  const table = useReactTable({
    data,
    columns,
    state: sortable ? { sorting } : undefined,
    onSortingChange: sortable ? setSorting : undefined,
    enableSorting: sortable,
    sortDescFirst: false,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: sortable ? getSortedRowModel() : undefined,
  });

  const leafColumns = table.getAllLeafColumns();
  const skeletonRows = Array.from({ length: loadingRows }, (_, n) => `skeleton-${n}`);

  return (
    <div className="overflow-x-auto rounded-lg border border-border bg-card shadow-xs">
      <table className="w-full border-collapse text-left text-sm">
        <thead className="border-b border-border bg-muted/70 text-xs font-semibold tracking-wider text-muted-foreground uppercase">
          {table.getHeaderGroups().map((headerGroup) => (
            <tr key={headerGroup.id}>
              {headerGroup.headers.map((header) => {
                const canSort = sortable && header.column.getCanSort();
                const sortDir = header.column.getIsSorted();
                const ariaSort =
                  sortDir === "asc" ? "ascending" : sortDir === "desc" ? "descending" : undefined;

                return (
                  <th key={header.id} aria-sort={ariaSort} className="px-3.5 py-2.5">
                    {header.isPlaceholder ? null : canSort ? (
                      <button
                        type="button"
                        onClick={header.column.getToggleSortingHandler()}
                        className="inline-flex cursor-pointer items-center gap-1.5 uppercase transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/50"
                      >
                        {flexRender(header.column.columnDef.header, header.getContext())}
                        {sortDir === "asc" ? (
                          <ArrowUp className="h-3.5 w-3.5 text-primary" aria-hidden="true" />
                        ) : sortDir === "desc" ? (
                          <ArrowDown className="h-3.5 w-3.5 text-primary" aria-hidden="true" />
                        ) : (
                          <ArrowUpDown className="h-3.5 w-3.5 opacity-40" aria-hidden="true" />
                        )}
                      </button>
                    ) : (
                      flexRender(header.column.columnDef.header, header.getContext())
                    )}
                  </th>
                );
              })}
            </tr>
          ))}
        </thead>
        <tbody className="divide-y divide-border">
          {loading ? (
            skeletonRows.map((rowKey) => (
              <tr key={rowKey} aria-busy="true">
                {leafColumns.map((column) => (
                  <td key={`${rowKey}-${column.id}`} className="px-3.5 py-3">
                    <div className="h-4 w-3/4 animate-pulse rounded bg-muted" />
                  </td>
                ))}
              </tr>
            ))
          ) : table.getRowModel().rows.length ? (
            table.getRowModel().rows.map((row) => (
              <tr key={row.id} className="transition-colors hover:bg-muted/40">
                {row.getVisibleCells().map((cell) => (
                  <td key={cell.id} className="px-3.5 py-2.5">
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </td>
                ))}
              </tr>
            ))
          ) : (
            <tr>
              <td colSpan={columns.length} className="px-4 py-10 text-center">
                <div className="flex flex-col items-center justify-center gap-1.5 text-muted-foreground">
                  {emptyIcon && <div className="mb-1 opacity-60">{emptyIcon}</div>}
                  <p className="text-sm font-medium text-foreground/80">{emptyMessage}</p>
                  {emptyDescription && (
                    <p className="max-w-sm text-xs text-muted-foreground">{emptyDescription}</p>
                  )}
                </div>
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
