import { ClientFileRow } from '../../models/response/responseLibrary';
import { filterCounts, filterImages, statusIcon } from './library-files.util';

const row = (name: string, status: ClientFileRow['status']): ClientFileRow =>
  ({ name, path: `/root/${name}`, rel: '', status });

describe('library files status helpers', () => {
  it('maps each status to its badge icon', () => {
    expect(statusIcon('loaded')).toBe('pi-check-circle');
    expect(statusIcon('failed')).toBe('pi-times-circle');
    expect(statusIcon('pending')).toBe('pi-clock');
  });

  it('filters images by status and keeps everything for "all"', () => {
    const images = [row('a.jpg', 'loaded'), row('b.jpg', 'failed'), row('c.jpg', 'pending'), row('d.jpg', 'loaded')];
    expect(filterImages(images, 'all').length).toBe(4);
    expect(filterImages(images, 'loaded').map(i => i.name)).toEqual(['a.jpg', 'd.jpg']);
    expect(filterImages(images, 'failed').map(i => i.name)).toEqual(['b.jpg']);
    expect(filterImages(images, 'pending').map(i => i.name)).toEqual(['c.jpg']);
  });

  it('maps the directory tallies onto the chip counts', () => {
    expect(filterCounts({ loaded: 5, failed: 2, pending: 1, total: 8 }))
      .toEqual({ all: 8, loaded: 5, failed: 2, pending: 1 });
  });
});
