import alpaca from "../assets/avatars/Alpaca.jpg";
import arcticFox from "../assets/avatars/Arcticfox.jpg";
import bear from "../assets/avatars/Bear.jpg";
import cheetah from "../assets/avatars/Cheetah.jpg";
import cheetah2 from "../assets/avatars/Cheetah2.jpg";
import chimpanzee from "../assets/avatars/Chimpanzee.jpg";
import dolphin from "../assets/avatars/Dolphin.jpg";
import fennec from "../assets/avatars/Fennec.jpg";
import gazelle from "../assets/avatars/Gazelle.jpg";
import gazelle2 from "../assets/avatars/Gazelle2.jpg";
import giraffe from "../assets/avatars/Giraffe.jpg";
import koala from "../assets/avatars/Koala.jpg";
import llama from "../assets/avatars/Llama.jpg";
import lynx from "../assets/avatars/Lynx.jpg";
import panda from "../assets/avatars/Panda.jpg";
import raccoon from "../assets/avatars/Raccoon.jpg";
import redPanda from "../assets/avatars/Redpanda.jpg";
import redPanda2 from "../assets/avatars/Redpanda2.jpg";
import retriever from "../assets/avatars/Retriever.jpg";
import shiba from "../assets/avatars/Shiba.jpg";
import snowLeopard from "../assets/avatars/Snowleopard.jpg";
import snowLeopard2 from "../assets/avatars/Snowleopard2.jpg";
import tiger from "../assets/avatars/Tiger.jpg";
import wolf from "../assets/avatars/Wolf.jpg";

export type Avatar = {
  id: string;
  name: string;
  src: string;
};

export const AVATARS: Avatar[] = [
  { id: "alpaca", name: "Alpaca", src: alpaca },
  { id: "arctic-fox", name: "Arctic fox", src: arcticFox },
  { id: "bear", name: "Bear", src: bear },
  { id: "cheetah", name: "Cheetah", src: cheetah },
  { id: "cheetah-2", name: "Cheetah", src: cheetah2 },
  { id: "chimpanzee", name: "Chimpanzee", src: chimpanzee },
  { id: "dolphin", name: "Dolphin", src: dolphin },
  { id: "fennec", name: "Fennec", src: fennec },
  { id: "gazelle", name: "Gazelle", src: gazelle },
  { id: "gazelle-2", name: "Gazelle", src: gazelle2 },
  { id: "giraffe", name: "Giraffe", src: giraffe },
  { id: "koala", name: "Koala", src: koala },
  { id: "llama", name: "Llama", src: llama },
  { id: "lynx", name: "Lynx", src: lynx },
  { id: "panda", name: "Panda", src: panda },
  { id: "raccoon", name: "Raccoon", src: raccoon },
  { id: "red-panda", name: "Red panda", src: redPanda },
  { id: "red-panda-2", name: "Red panda", src: redPanda2 },
  { id: "retriever", name: "Retriever", src: retriever },
  { id: "shiba", name: "Shiba", src: shiba },
  { id: "snow-leopard", name: "Snow leopard", src: snowLeopard },
  { id: "snow-leopard-2", name: "Snow leopard", src: snowLeopard2 },
  { id: "tiger", name: "Tiger", src: tiger },
  { id: "wolf", name: "Wolf", src: wolf },
];

const byId = new Map(AVATARS.map((avatar) => [avatar.id, avatar]));

export function avatarById(id: string | undefined): Avatar | undefined {
  if (!id) return undefined;
  return byId.get(id);
}
