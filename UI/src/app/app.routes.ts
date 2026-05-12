import { Routes } from '@angular/router';
import { Login } from './views/login/login';
import { Master } from './views/master/master';
import { Search } from './views/search/search';

export const routes: Routes = [
    {path:'',component:Login},
    {path:'master',component:Master,children:[
        {path:'search',component:Search}
    ]},
    
];
