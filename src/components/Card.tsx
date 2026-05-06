import type { PropsWithChildren } from 'react';
import { Link, useLocation } from 'react-router-dom';

export interface CardProps {
  className?: string;
  bgColor?: 'white' | 'black';
  links?: LinkProps[];
}

export interface LinkProps {
  to: string;
  label: string;
}

export const Card = ({ bgColor = 'white', className = '', links, children }: PropsWithChildren<CardProps>) => (
  <div
    className={`rounded shadow border-2 ${bgColor === 'white' ? 'bg-white border-grey-light' : 'bg-black border-black'} ${className}`}
  >
    {links && (
      <div className="px-6 md:px-12 py-2 border-b-2 border-grey-light flex flex-row mb-8 overflow-x-auto">
        {links.map(({ to, label }) => (
          <LinkItem to={to} key={to}>
            {label}
          </LinkItem>
        ))}
      </div>
    )}
    {children}
  </div>
);

const LinkItem = ({ to, children }: PropsWithChildren<{ to: string }>) => {
  const { pathname } = useLocation();
  const selected = pathname === to;

  return (
    <Link
      to={to}
      className={`relative text-base font-semibold px-4 mr-2 py-2 transition hover:text-black ${selected ? 'text-black' : 'text-grey'}`}
    >
      {children}
      {selected && (
        <span className="absolute bottom-0 left-4 right-4 h-0.5 bg-gradient-to-r from-cyan to-magenta rounded-full" />
      )}
    </Link>
  );
};
