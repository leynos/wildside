/**
 * @file Root application component.
 */
import { useQuery } from '@tanstack/react-query';
import { listUsers } from '../api/client';

export function App() {
  const { data } = useQuery({ queryKey: ['users'], queryFn: () => listUsers() });
  return (
    <div className="p-6 min-h-screen bg-base-200 text-base-content">
      <div className="navbar bg-base-100 rounded-box mb-6">
        <a className="btn btn-ghost text-xl">myapp</a>
      </div>
      <ul className="menu bg-base-100 rounded-box">
        {(data ?? []).map(u => <li key={u.id}><a>{u.display_name}</a></li>)}
      </ul>
    </div>
  );
}
