import { NgModule } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MenuModule } from 'primeng/menu';
import { AvatarModule } from 'primeng/avatar';
import { BadgeModule } from 'primeng/badge';
import { ButtonModule } from 'primeng/button';
import { RouterModule, RouterOutlet } from '@angular/router';
import { SelectModule } from 'primeng/select';
import { FieldsetModule } from 'primeng/fieldset';
import { FileUploadModule } from 'primeng/fileupload';
import { CardModule } from 'primeng/card';
import { ImageModule } from 'primeng/image';
import { TreeModule } from 'primeng/tree';
import { DividerModule } from 'primeng/divider';
import { DataViewModule } from 'primeng/dataview';
import { TimelineModule } from 'primeng/timeline';
import { StepperModule } from 'primeng/stepper';
import { InputTextModule } from 'primeng/inputtext';
import { PasswordModule } from 'primeng/password';
import { ToastModule } from 'primeng/toast';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';
import { DialogModule } from 'primeng/dialog';
import { IconFieldModule } from 'primeng/iconfield';
import { InputIconModule } from 'primeng/inputicon';
import { TextareaModule } from 'primeng/textarea';
import { ProgressSpinnerModule } from 'primeng/progressspinner';
import { PanelMenuModule } from 'primeng/panelmenu';
import { RippleModule } from 'primeng/ripple';
@NgModule({
  declarations: [],
  imports: [
    CommonModule,MenuModule,AvatarModule,BadgeModule,ButtonModule,RouterModule,SelectModule,FieldsetModule,FileUploadModule,RouterOutlet,CardModule,ImageModule,TreeModule,DividerModule,DataViewModule,TimelineModule,StepperModule,InputTextModule,PasswordModule,ToastModule,TableModule,TagModule,DialogModule,IconFieldModule,InputIconModule,TextareaModule,ProgressSpinnerModule,PanelMenuModule, RippleModule
  ],exports:[MenuModule,AvatarModule,BadgeModule,ButtonModule,RouterModule,SelectModule,FieldsetModule,FileUploadModule,RouterOutlet,CardModule,ImageModule,TreeModule,DividerModule,DataViewModule,TimelineModule,StepperModule,InputTextModule,PasswordModule,ToastModule,TableModule,TagModule,DialogModule,IconFieldModule,InputIconModule,TextareaModule,ProgressSpinnerModule,PanelMenuModule,RippleModule]
})
export class PrimengComponentsModule { }
