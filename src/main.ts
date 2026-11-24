import { mount } from 'svelte';
import App from './App.svelte';
import './app.css';

// В релизе отключаем браузерное контекстное меню («Обновить», «Просмотреть код» и т. п.).
if (import.meta.env.PROD) {
  document.addEventListener('contextmenu', (e) => e.preventDefault());
}

const target = document.getElementById('app');
if (!target) throw new Error('Не найден контейнер #app');

export default mount(App, { target });
