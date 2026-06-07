import { Routes } from '@angular/router';
import { Login } from './views/login/login';
import { Master } from './views/master/master';
import { Search } from './views/search/search';
import { Profile } from './views/profile/profile';
import { Library } from './views/library/library';
import { Browse } from './views/browse/browse';
import { Catalog } from './views/catalog/catalog';
import { Editor } from './views/catalog/theme-builder';
import { authGuard } from './guards/auth.guard';
import { loginGuard } from './guards/login.guard';
import { subscriptionGuard } from './guards/subscription.guard';

export const routes: Routes = [
    { path: '', component: Login, canActivate: [loginGuard] },
    { path: 'master', component: Master, canActivate: [authGuard], children: [
        { path: '', redirectTo: 'search', pathMatch: 'full' },
        // Feature pages require an active subscription (trial/active).
        { path: 'search',         component: Search,  canActivate: [subscriptionGuard] },
        { path: 'library',        component: Library, canActivate: [subscriptionGuard] },
        { path: 'browse',         component: Browse,  canActivate: [subscriptionGuard] },
        { path: 'catalog',        component: Catalog, canActivate: [subscriptionGuard] },
        { path: 'catalog/editor', component: Editor,  canActivate: [subscriptionGuard] },
        // Profile stays open so expired users can renew.
        { path: 'profile',        component: Profile },
    ]},
];
