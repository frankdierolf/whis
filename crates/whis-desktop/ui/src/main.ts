import { createVaporApp } from 'vue';
import App from './App.vue';

type VaporRoot = Parameters<typeof createVaporApp>[0];
const RootComponent = App as unknown as VaporRoot;

createVaporApp(RootComponent).mount('#app');
