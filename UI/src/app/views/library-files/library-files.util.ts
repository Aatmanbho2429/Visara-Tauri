import { ClientFileRow, ClientFileStatus, responseLibraryDirCounts } from '../../models/response/responseLibrary';

// Filter chip value: one load status, or every image.
export type StatusFilter = 'all' | ClientFileStatus;

// PrimeIcon shown in an image's corner badge for each load status.
const STATUS_ICON: Record<ClientFileStatus, string> = {
  loaded:  'pi-check-circle',
  failed:  'pi-times-circle',
  pending: 'pi-clock',
};

export function statusIcon(status: ClientFileStatus): string {
  return STATUS_ICON[status];
}

// Images matching the active chip; `all` keeps everything.
export function filterImages(images: ClientFileRow[], filter: StatusFilter): ClientFileRow[] {
  return filter === 'all' ? images : images.filter(i => i.status === filter);
}

// Count shown on each chip, straight from the directory's status tallies.
export function filterCounts(counts: responseLibraryDirCounts): Record<StatusFilter, number> {
  return { all: counts.total, loaded: counts.loaded, failed: counts.failed, pending: counts.pending };
}
