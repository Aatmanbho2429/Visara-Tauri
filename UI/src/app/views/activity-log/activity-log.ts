import { Component } from '@angular/core';
import { CommonModule } from '@angular/common';
import { TranslateModule } from '@ngx-translate/core';

interface LogEntry {
  id: number;
  image: string;
  folder: string;
  results: number;
  time: string;
  gradient: string;
}

@Component({
  selector: 'app-activity-log',
  imports: [CommonModule, TranslateModule],
  templateUrl: './activity-log.html',
  styleUrl: './activity-log.scss',
})
export class ActivityLog {
  logs: LogEntry[] = [
    { id: 1, image: 'hero_banner.jpg',    folder: 'C:\\Designs\\Web',          results: 24, time: '2 mins ago',  gradient: 'linear-gradient(135deg,#d946ef,#fb923c)' },
    { id: 2, image: 'logo_v3.png',        folder: 'C:\\Designs\\Brand',         results: 12, time: '1 hour ago',  gradient: 'linear-gradient(135deg,#a21caf,#f97316)' },
    { id: 3, image: 'product_shot.jpg',   folder: 'C:\\Photos\\Products',       results: 0,  time: '3 hours ago', gradient: 'linear-gradient(135deg,#c026d3,#ea580c)' },
    { id: 4, image: 'texture_marble.jpg', folder: 'C:\\Assets\\Textures',       results: 47, time: 'Yesterday',   gradient: 'linear-gradient(135deg,#86198f,#d946ef)' },
    { id: 5, image: 'icon_set.png',       folder: 'C:\\Designs\\Icons',         results: 8,  time: 'Yesterday',   gradient: 'linear-gradient(135deg,#701a75,#fb923c)' },
    { id: 6, image: 'bg_abstract.jpg',    folder: 'C:\\Stock\\Backgrounds',     results: 31, time: '2 days ago',  gradient: 'linear-gradient(135deg,#e879f9,#f97316)' },
  ];
}
