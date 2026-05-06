interface IdenticonProps {
  address: string;
  size?: 'sm' | 'md';
}

const Identicon = ({ address, size = 'md' }: IdenticonProps) => (
  <div className={`avatar bg-white rounded-full ${size === 'sm' ? 'border-2' : 'border-4'}`}>
    <div className={size === 'sm' ? 'w-8 p-[0.4rem]' : 'w-14 p-[.8rem]'}>
      <img src={`https://id.lobstr.co/${address}.png`} alt="identicon" />
    </div>
  </div>
);

export default Identicon;
