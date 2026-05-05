import { LinkButton, NativeLinkButton } from '@components/Button';
import CardsImage from '@images/cards.jpg';
import Konsta from '@images/konsta.jpeg';
import ReflectorLogo from '@images/reflector.svg';
import SCFLogo from '@images/scf.svg';
import Teemu from '@images/teemu.jpg';
import { FaDiscord, FaGithub } from 'react-icons/fa';

const team = [
  {
    image: Teemu.src,
    name: 'Teemu Hynnä',
    discord: '.h0suja',
    github: 'teolhyn',
  },
  {
    image: Konsta.src,
    name: 'Konsta Purtsi',
    discord: 'konsta',
    github: 'kovipu',
  },
];

const LandingPage = () => (
  <>
    <section className="flex flex-row flex-wrap-reverse items-center">
      <div className="md:w-1/2">
        <h1 className="text-2xl md:text-[42px] font-medium mb-8 tracking-tight">
          A trustless loan platform focusing on single-token lending pools.
        </h1>
        <h2 className="text-2xl md:text-[42px] font-bold mb-8">DeFi made simple.</h2>
        <LinkButton to="/lend" className="text-lg">
          Open the app
        </LinkButton>
      </div>
      <div className="md:w-1/2">
        <img src={CardsImage.src} alt="" />
      </div>
    </section>
    <section className="flex flex-row flex-wrap my-10 md:mb-24">
      <Award image={SCFLogo.src} title="Stellar Community Fund" subtitle="Kickstart Award & Build Award" />
      <Award image={ReflectorLogo.src} title="Reflector Dev Contest" subtitle="Best Oracle-based Smart Contract" />
    </section>
    <section className="flex flex-row flex-wrap my-10">
      <div className="md:w-1/2 py-14 md:pr-14 border-t-4">
        <p className="text-lg md:text-2xl leading-relaxed mb-8 tracking-tight">
          Laina is currently in the early development stage and is only operating on the Stellar Testnet.
        </p>
        <p className="text-lg md:text-2xl leading-relaxed mb-8">We are source-available, check it out!</p>
        <NativeLinkButton to="https://github.com/laina-defi/laina" className="text-lg">
          View the source
        </NativeLinkButton>
      </div>
      <div className="md:w-1/2 py-7 md:pt-14 pl-7 md:pl-14 border-t-4 border-l-4">
        <h3 className="text-xl font-bold mb-6 md:mb-8">Meet Laina.</h3>
        <p className="md:text-lg mb-6 md:mb-8 leading-relaxed">
          Laina is a low fee, trustless, and easy-to-use decentralized loan platform.
        </p>
        <p className="md:text-lg leading-relaxed">
          We are focusing on making a simple and efficient DeFi product, where there is minimal need for token swapping
          or liquidity other than what is used for lending. Our vision is to change DeFi by making it accessible and
          understandable for everyone, regardless of their technical knowledge or financial status. By providing
          efficient single-token lending pools, we eliminate the complexities often associated with multi-token systems.
        </p>
      </div>
    </section>
    <section className="flex flex-row flex-wrap my-20">
      <div className="md:w-1/2">
        <h3 className="text-2xl md:text-[42px] font-bold mb-7 md:mb-14 tracking-tight">The Team</h3>
        <div className="flex flex-row gap-8">
          {team.map((member) => (
            <TeamMember key={member.name} {...member} />
          ))}
        </div>
      </div>
      <div className="md:w-1/2 pt-8 md:py-14 md:pl-14 my-auto">
        <p className="text-lg md:text-xl mb-8 leading-relaxed tracking-tight">Got a question? Ask us in our Discord!</p>
        <NativeLinkButton to="https://discord.gg/gnUAFr3fUv" className="text-lg">
          Join Laina's Discord
        </NativeLinkButton>
      </div>
    </section>
  </>
);

interface AwardProps {
  image: string;
  title: string;
  subtitle: string;
}

const Award = ({ image, title, subtitle }: AwardProps) => (
  <div className="flex items-center md:w-1/2 my-4 md:px-14">
    <img src={image} alt="" className="h-14 w-14 md:h-16 md:w-16 mr-4" />
    <div>
      <p className="text-lg md:text-xl font-semibold tracking-tight">{title}</p>
      <p className="md:text-lg tracking-tight">{subtitle}</p>
    </div>
  </div>
);

interface TeamMemberProps {
  image: string;
  name: string;
  discord: string;
  github: string;
}

const TeamMember = ({ image, name, discord, github }: TeamMemberProps) => (
  <div className="md:mr-14">
    <div className="aspect-square rounded mb-3 md:mb-8 overflow-hidden">
      <img src={image} alt={name} />
    </div>
    <p className="text-lg md:text-xl font-semibold">{name}</p>
    <span className="flex flex-row items-center md:text-lg my-2">
      <FaDiscord className="w-8 h-8 mr-2" />
      {discord}
    </span>
    <span className="flex flex-row items-center md:text-lg my-2">
      <FaGithub className="w-8 h-8 mr-2" />
      {github}
    </span>
  </div>
);

export default LandingPage;
