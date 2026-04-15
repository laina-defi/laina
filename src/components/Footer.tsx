import type { ComponentType } from 'react';
import { PiDiscordLogoFill, PiGithubLogoFill, PiXLogoFill } from 'react-icons/pi';

export default function Footer() {
  return (
    <footer className="bg-black px-4 flex flex-col items-center justify-center">
      <div className="flex flex-row w-[74rem] max-w-full flex-wrap items-center mb-20">
        <div className="md:w-1/2 flex flex-col pt-10 px-4">
          <SocialLink href="https://discord.com/invite/gnUAFr3fUv" text="Laina DeFi" Icon={PiDiscordLogoFill} />
          <SocialLink href="https://x.com/Lainadefi" text="@Lainadefi" Icon={PiXLogoFill} />
          <SocialLink href="https://github.com/laina-defi" text="laina-defi" Icon={PiGithubLogoFill} />
        </div>
        <div className="md:w-1/2 pt-10 px-4 md:pl-14">
          <p className="text-white max-w-full text-lg">This website does not use cookies.</p>
          <p className="text-white max-w-full text-sm">&copy; Copyright 2025, Laina Protocol</p>
        </div>
      </div>
    </footer>
  );
}

interface IconProps {
  className?: string;
  size?: number;
}

const SocialLink = ({ href, text, Icon }: { href: string; text: string; Icon: ComponentType<IconProps> }) => {
  return (
    <a
      className="text-lg text-white cursor-pointer flex flex-row w-max my-1 hover:text-grey"
      href={href}
      target="_blank"
      rel="noreferrer"
    >
      <Icon className="mr-2" size={32} />
      {text}
    </a>
  );
};
