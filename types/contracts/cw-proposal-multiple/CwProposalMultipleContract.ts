/**
* This file was automatically generated by @cosmwasm/ts-codegen@0.5.8.
* DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
* and run the @cosmwasm/ts-codegen generate command to regenerate this file.
*/

import { CosmWasmClient, ExecuteResult, SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { StdFee } from "@cosmjs/amino";
export type Addr = string;
export type Uint128 = string;
export type Duration = {
  height: number;
} | {
  time: number;
};
export type VotingStrategy = {
  single_choice: {
    quorum: PercentageThreshold;
    [k: string]: unknown;
  };
};
export type PercentageThreshold = {
  majority: {
    [k: string]: unknown;
  };
} | {
  percent: Decimal;
};
export type Decimal = string;
export interface ConfigResponse {
  allow_revoting: boolean;
  close_proposal_on_execution_failure: boolean;
  dao: Addr;
  deposit_info?: CheckedDepositInfo | null;
  max_voting_period: Duration;
  min_voting_period?: Duration | null;
  only_members_execute: boolean;
  voting_strategy: VotingStrategy;
  [k: string]: unknown;
}
export interface CheckedDepositInfo {
  deposit: Uint128;
  refund_failed_proposals: boolean;
  token: Addr;
  [k: string]: unknown;
}
export type ExecuteMsg = {
  propose: {
    choices: MultipleChoiceOptions;
    description: string;
    title: string;
    [k: string]: unknown;
  };
} | {
  vote: {
    proposal_id: number;
    vote: MultipleChoiceVote;
    [k: string]: unknown;
  };
} | {
  execute: {
    proposal_id: number;
    [k: string]: unknown;
  };
} | {
  close: {
    proposal_id: number;
    [k: string]: unknown;
  };
} | {
  update_config: {
    allow_revoting: boolean;
    close_proposal_on_execution_failure: boolean;
    dao: string;
    deposit_info?: DepositInfo | null;
    max_voting_period: Duration;
    min_voting_period?: Duration | null;
    only_members_execute: boolean;
    voting_strategy: VotingStrategy;
    [k: string]: unknown;
  };
} | {
  add_proposal_hook: {
    address: string;
    [k: string]: unknown;
  };
} | {
  remove_proposal_hook: {
    address: string;
    [k: string]: unknown;
  };
} | {
  add_vote_hook: {
    address: string;
    [k: string]: unknown;
  };
} | {
  remove_vote_hook: {
    address: string;
    [k: string]: unknown;
  };
};
export type CosmosMsgForEmpty = {
  bank: BankMsg;
} | {
  custom: Empty;
} | {
  staking: StakingMsg;
} | {
  distribution: DistributionMsg;
} | {
  stargate: {
    type_url: string;
    value: Binary;
    [k: string]: unknown;
  };
} | {
  ibc: IbcMsg;
} | {
  wasm: WasmMsg;
} | {
  gov: GovMsg;
};
export type BankMsg = {
  send: {
    amount: Coin[];
    to_address: string;
    [k: string]: unknown;
  };
} | {
  burn: {
    amount: Coin[];
    [k: string]: unknown;
  };
};
export type StakingMsg = {
  delegate: {
    amount: Coin;
    validator: string;
    [k: string]: unknown;
  };
} | {
  undelegate: {
    amount: Coin;
    validator: string;
    [k: string]: unknown;
  };
} | {
  redelegate: {
    amount: Coin;
    dst_validator: string;
    src_validator: string;
    [k: string]: unknown;
  };
};
export type DistributionMsg = {
  set_withdraw_address: {
    address: string;
    [k: string]: unknown;
  };
} | {
  withdraw_delegator_reward: {
    validator: string;
    [k: string]: unknown;
  };
};
export type Binary = string;
export type IbcMsg = {
  transfer: {
    amount: Coin;
    channel_id: string;
    timeout: IbcTimeout;
    to_address: string;
    [k: string]: unknown;
  };
} | {
  send_packet: {
    channel_id: string;
    data: Binary;
    timeout: IbcTimeout;
    [k: string]: unknown;
  };
} | {
  close_channel: {
    channel_id: string;
    [k: string]: unknown;
  };
};
export type Timestamp = Uint64;
export type Uint64 = string;
export type WasmMsg = {
  execute: {
    contract_addr: string;
    funds: Coin[];
    msg: Binary;
    [k: string]: unknown;
  };
} | {
  instantiate: {
    admin?: string | null;
    code_id: number;
    funds: Coin[];
    label: string;
    msg: Binary;
    [k: string]: unknown;
  };
} | {
  migrate: {
    contract_addr: string;
    msg: Binary;
    new_code_id: number;
    [k: string]: unknown;
  };
} | {
  update_admin: {
    admin: string;
    contract_addr: string;
    [k: string]: unknown;
  };
} | {
  clear_admin: {
    contract_addr: string;
    [k: string]: unknown;
  };
};
export type GovMsg = {
  vote: {
    proposal_id: number;
    vote: VoteOption;
    [k: string]: unknown;
  };
};
export type VoteOption = "yes" | "no" | "abstain" | "no_with_veto";
export type DepositToken = {
  token: {
    address: string;
    [k: string]: unknown;
  };
} | {
  voting_module_token: {
    [k: string]: unknown;
  };
};
export interface MultipleChoiceOptions {
  options: MultipleChoiceOption[];
  [k: string]: unknown;
}
export interface MultipleChoiceOption {
  description: string;
  msgs?: CosmosMsgForEmpty[] | null;
  [k: string]: unknown;
}
export interface Coin {
  amount: Uint128;
  denom: string;
  [k: string]: unknown;
}
export interface Empty {
  [k: string]: unknown;
}
export interface IbcTimeout {
  block?: IbcTimeoutBlock | null;
  timestamp?: Timestamp | null;
  [k: string]: unknown;
}
export interface IbcTimeoutBlock {
  height: number;
  revision: number;
  [k: string]: unknown;
}
export interface MultipleChoiceVote {
  option_id: number;
  [k: string]: unknown;
}
export interface DepositInfo {
  deposit: Uint128;
  refund_failed_proposals: boolean;
  token: DepositToken;
  [k: string]: unknown;
}
export interface GetVoteResponse {
  vote?: VoteInfo | null;
  [k: string]: unknown;
}
export interface VoteInfo {
  power: Uint128;
  vote: MultipleChoiceVote;
  voter: Addr;
  [k: string]: unknown;
}
export type GovernanceModulesResponse = Addr[];
export interface InfoResponse {
  info: ContractVersion;
  [k: string]: unknown;
}
export interface ContractVersion {
  contract: string;
  version: string;
  [k: string]: unknown;
}
export interface InstantiateMsg {
  allow_revoting: boolean;
  close_proposal_on_execution_failure: boolean;
  deposit_info?: DepositInfo | null;
  max_voting_period: Duration;
  min_voting_period?: Duration | null;
  only_members_execute: boolean;
  voting_strategy: VotingStrategy;
  [k: string]: unknown;
}
export type MultipleChoiceOptionType = "None" | "Standard";
export type Expiration = {
  at_height: number;
} | {
  at_time: Timestamp;
} | {
  never: {
    [k: string]: unknown;
  };
};
export type Status = "open" | "rejected" | "passed" | "executed" | "closed" | "execution_failed";
export interface ListProposalsResponse {
  proposals: ProposalResponse[];
  [k: string]: unknown;
}
export interface ProposalResponse {
  id: number;
  proposal: MultipleChoiceProposal;
  [k: string]: unknown;
}
export interface MultipleChoiceProposal {
  allow_revoting: boolean;
  choices: CheckedMultipleChoiceOption[];
  created: Timestamp;
  deposit_info?: CheckedDepositInfo | null;
  description: string;
  expiration: Expiration;
  last_updated: Timestamp;
  min_voting_period?: Expiration | null;
  proposer: Addr;
  start_height: number;
  status: Status;
  title: string;
  total_power: Uint128;
  votes: MultipleChoiceVotes;
  voting_strategy: VotingStrategy;
  [k: string]: unknown;
}
export interface CheckedMultipleChoiceOption {
  description: string;
  index: number;
  msgs?: CosmosMsgForEmpty[] | null;
  option_type: MultipleChoiceOptionType;
  vote_count: Uint128;
  [k: string]: unknown;
}
export interface MultipleChoiceVotes {
  vote_weights: Uint128[];
  [k: string]: unknown;
}
export interface ListVotesResponse {
  votes: VoteInfo[];
  [k: string]: unknown;
}
export interface MigrateMsg {
  [k: string]: unknown;
}
export type ProposalCountResponse = number;
export interface ProposalHooksResponse {
  hooks: string[];
  [k: string]: unknown;
}
export type QueryMsg = {
  config: {
    [k: string]: unknown;
  };
} | {
  proposal: {
    proposal_id: number;
    [k: string]: unknown;
  };
} | {
  list_proposals: {
    limit?: number | null;
    start_after?: number | null;
    [k: string]: unknown;
  };
} | {
  reverse_proposals: {
    limit?: number | null;
    start_before?: number | null;
    [k: string]: unknown;
  };
} | {
  proposal_count: {
    [k: string]: unknown;
  };
} | {
  get_vote: {
    proposal_id: number;
    voter: string;
    [k: string]: unknown;
  };
} | {
  list_votes: {
    limit?: number | null;
    proposal_id: number;
    start_after?: string | null;
    [k: string]: unknown;
  };
} | {
  proposal_hooks: {
    [k: string]: unknown;
  };
} | {
  vote_hooks: {
    [k: string]: unknown;
  };
} | {
  info: {
    [k: string]: unknown;
  };
};
export interface ReverseProposalsResponse {
  proposals: ProposalResponse[];
  [k: string]: unknown;
}
export interface VoteHooksResponse {
  hooks: string[];
  [k: string]: unknown;
}
export interface VoteResponse {
  vote?: VoteInfo | null;
  [k: string]: unknown;
}
export interface CwProposalMultipleReadOnlyInterface {
  contractAddress: string;
  config: () => Promise<ConfigResponse>;
  proposal: ({
    proposalId
  }: {
    proposalId: number;
  }) => Promise<ProposalResponse>;
  listProposals: ({
    limit,
    startAfter
  }: {
    limit?: number;
    startAfter?: number;
  }) => Promise<ListProposalsResponse>;
  reverseProposals: ({
    limit,
    startBefore
  }: {
    limit?: number;
    startBefore?: number;
  }) => Promise<ReverseProposalsResponse>;
  proposalCount: () => Promise<ProposalCountResponse>;
  getVote: ({
    proposalId,
    voter
  }: {
    proposalId: number;
    voter: string;
  }) => Promise<GetVoteResponse>;
  listVotes: ({
    limit,
    proposalId,
    startAfter
  }: {
    limit?: number;
    proposalId: number;
    startAfter?: string;
  }) => Promise<ListVotesResponse>;
  proposalHooks: () => Promise<ProposalHooksResponse>;
  voteHooks: () => Promise<VoteHooksResponse>;
  info: () => Promise<InfoResponse>;
}
export class CwProposalMultipleQueryClient implements CwProposalMultipleReadOnlyInterface {
  client: CosmWasmClient;
  contractAddress: string;

  constructor(client: CosmWasmClient, contractAddress: string) {
    this.client = client;
    this.contractAddress = contractAddress;
    this.config = this.config.bind(this);
    this.proposal = this.proposal.bind(this);
    this.listProposals = this.listProposals.bind(this);
    this.reverseProposals = this.reverseProposals.bind(this);
    this.proposalCount = this.proposalCount.bind(this);
    this.getVote = this.getVote.bind(this);
    this.listVotes = this.listVotes.bind(this);
    this.proposalHooks = this.proposalHooks.bind(this);
    this.voteHooks = this.voteHooks.bind(this);
    this.info = this.info.bind(this);
  }

  config = async (): Promise<ConfigResponse> => {
    return this.client.queryContractSmart(this.contractAddress, {
      config: {}
    });
  };
  proposal = async ({
    proposalId
  }: {
    proposalId: number;
  }): Promise<ProposalResponse> => {
    return this.client.queryContractSmart(this.contractAddress, {
      proposal: {
        proposal_id: proposalId
      }
    });
  };
  listProposals = async ({
    limit,
    startAfter
  }: {
    limit?: number;
    startAfter?: number;
  }): Promise<ListProposalsResponse> => {
    return this.client.queryContractSmart(this.contractAddress, {
      list_proposals: {
        limit,
        start_after: startAfter
      }
    });
  };
  reverseProposals = async ({
    limit,
    startBefore
  }: {
    limit?: number;
    startBefore?: number;
  }): Promise<ReverseProposalsResponse> => {
    return this.client.queryContractSmart(this.contractAddress, {
      reverse_proposals: {
        limit,
        start_before: startBefore
      }
    });
  };
  proposalCount = async (): Promise<ProposalCountResponse> => {
    return this.client.queryContractSmart(this.contractAddress, {
      proposal_count: {}
    });
  };
  getVote = async ({
    proposalId,
    voter
  }: {
    proposalId: number;
    voter: string;
  }): Promise<GetVoteResponse> => {
    return this.client.queryContractSmart(this.contractAddress, {
      get_vote: {
        proposal_id: proposalId,
        voter
      }
    });
  };
  listVotes = async ({
    limit,
    proposalId,
    startAfter
  }: {
    limit?: number;
    proposalId: number;
    startAfter?: string;
  }): Promise<ListVotesResponse> => {
    return this.client.queryContractSmart(this.contractAddress, {
      list_votes: {
        limit,
        proposal_id: proposalId,
        start_after: startAfter
      }
    });
  };
  proposalHooks = async (): Promise<ProposalHooksResponse> => {
    return this.client.queryContractSmart(this.contractAddress, {
      proposal_hooks: {}
    });
  };
  voteHooks = async (): Promise<VoteHooksResponse> => {
    return this.client.queryContractSmart(this.contractAddress, {
      vote_hooks: {}
    });
  };
  info = async (): Promise<InfoResponse> => {
    return this.client.queryContractSmart(this.contractAddress, {
      info: {}
    });
  };
}
export interface CwProposalMultipleInterface extends CwProposalMultipleReadOnlyInterface {
  contractAddress: string;
  sender: string;
  propose: ({
    choices,
    description,
    title
  }: {
    choices: MultipleChoiceOptions;
    description: string;
    title: string;
  }, fee?: number | StdFee | "auto", memo?: string, funds?: readonly Coin[]) => Promise<ExecuteResult>;
  vote: ({
    proposalId,
    vote
  }: {
    proposalId: number;
    vote: MultipleChoiceVote;
  }, fee?: number | StdFee | "auto", memo?: string, funds?: readonly Coin[]) => Promise<ExecuteResult>;
  execute: ({
    proposalId
  }: {
    proposalId: number;
  }, fee?: number | StdFee | "auto", memo?: string, funds?: readonly Coin[]) => Promise<ExecuteResult>;
  close: ({
    proposalId
  }: {
    proposalId: number;
  }, fee?: number | StdFee | "auto", memo?: string, funds?: readonly Coin[]) => Promise<ExecuteResult>;
  updateConfig: ({
    allowRevoting,
    closeProposalOnExecutionFailure,
    dao,
    depositInfo,
    maxVotingPeriod,
    minVotingPeriod,
    onlyMembersExecute,
    votingStrategy
  }: {
    allowRevoting: boolean;
    closeProposalOnExecutionFailure: boolean;
    dao: string;
    depositInfo?: DepositInfo;
    maxVotingPeriod: Duration;
    minVotingPeriod?: Duration;
    onlyMembersExecute: boolean;
    votingStrategy: VotingStrategy;
  }, fee?: number | StdFee | "auto", memo?: string, funds?: readonly Coin[]) => Promise<ExecuteResult>;
  addProposalHook: ({
    address
  }: {
    address: string;
  }, fee?: number | StdFee | "auto", memo?: string, funds?: readonly Coin[]) => Promise<ExecuteResult>;
  removeProposalHook: ({
    address
  }: {
    address: string;
  }, fee?: number | StdFee | "auto", memo?: string, funds?: readonly Coin[]) => Promise<ExecuteResult>;
  addVoteHook: ({
    address
  }: {
    address: string;
  }, fee?: number | StdFee | "auto", memo?: string, funds?: readonly Coin[]) => Promise<ExecuteResult>;
  removeVoteHook: ({
    address
  }: {
    address: string;
  }, fee?: number | StdFee | "auto", memo?: string, funds?: readonly Coin[]) => Promise<ExecuteResult>;
}
export class CwProposalMultipleClient extends CwProposalMultipleQueryClient implements CwProposalMultipleInterface {
  client: SigningCosmWasmClient;
  sender: string;
  contractAddress: string;

  constructor(client: SigningCosmWasmClient, sender: string, contractAddress: string) {
    super(client, contractAddress);
    this.client = client;
    this.sender = sender;
    this.contractAddress = contractAddress;
    this.propose = this.propose.bind(this);
    this.vote = this.vote.bind(this);
    this.execute = this.execute.bind(this);
    this.close = this.close.bind(this);
    this.updateConfig = this.updateConfig.bind(this);
    this.addProposalHook = this.addProposalHook.bind(this);
    this.removeProposalHook = this.removeProposalHook.bind(this);
    this.addVoteHook = this.addVoteHook.bind(this);
    this.removeVoteHook = this.removeVoteHook.bind(this);
  }

  propose = async ({
    choices,
    description,
    title
  }: {
    choices: MultipleChoiceOptions;
    description: string;
    title: string;
  }, fee: number | StdFee | "auto" = "auto", memo?: string, funds?: readonly Coin[]): Promise<ExecuteResult> => {
    return await this.client.execute(this.sender, this.contractAddress, {
      propose: {
        choices,
        description,
        title
      }
    }, fee, memo, funds);
  };
  vote = async ({
    proposalId,
    vote
  }: {
    proposalId: number;
    vote: MultipleChoiceVote;
  }, fee: number | StdFee | "auto" = "auto", memo?: string, funds?: readonly Coin[]): Promise<ExecuteResult> => {
    return await this.client.execute(this.sender, this.contractAddress, {
      vote: {
        proposal_id: proposalId,
        vote
      }
    }, fee, memo, funds);
  };
  execute = async ({
    proposalId
  }: {
    proposalId: number;
  }, fee: number | StdFee | "auto" = "auto", memo?: string, funds?: readonly Coin[]): Promise<ExecuteResult> => {
    return await this.client.execute(this.sender, this.contractAddress, {
      execute: {
        proposal_id: proposalId
      }
    }, fee, memo, funds);
  };
  close = async ({
    proposalId
  }: {
    proposalId: number;
  }, fee: number | StdFee | "auto" = "auto", memo?: string, funds?: readonly Coin[]): Promise<ExecuteResult> => {
    return await this.client.execute(this.sender, this.contractAddress, {
      close: {
        proposal_id: proposalId
      }
    }, fee, memo, funds);
  };
  updateConfig = async ({
    allowRevoting,
    closeProposalOnExecutionFailure,
    dao,
    depositInfo,
    maxVotingPeriod,
    minVotingPeriod,
    onlyMembersExecute,
    votingStrategy
  }: {
    allowRevoting: boolean;
    closeProposalOnExecutionFailure: boolean;
    dao: string;
    depositInfo?: DepositInfo;
    maxVotingPeriod: Duration;
    minVotingPeriod?: Duration;
    onlyMembersExecute: boolean;
    votingStrategy: VotingStrategy;
  }, fee: number | StdFee | "auto" = "auto", memo?: string, funds?: readonly Coin[]): Promise<ExecuteResult> => {
    return await this.client.execute(this.sender, this.contractAddress, {
      update_config: {
        allow_revoting: allowRevoting,
        close_proposal_on_execution_failure: closeProposalOnExecutionFailure,
        dao,
        deposit_info: depositInfo,
        max_voting_period: maxVotingPeriod,
        min_voting_period: minVotingPeriod,
        only_members_execute: onlyMembersExecute,
        voting_strategy: votingStrategy
      }
    }, fee, memo, funds);
  };
  addProposalHook = async ({
    address
  }: {
    address: string;
  }, fee: number | StdFee | "auto" = "auto", memo?: string, funds?: readonly Coin[]): Promise<ExecuteResult> => {
    return await this.client.execute(this.sender, this.contractAddress, {
      add_proposal_hook: {
        address
      }
    }, fee, memo, funds);
  };
  removeProposalHook = async ({
    address
  }: {
    address: string;
  }, fee: number | StdFee | "auto" = "auto", memo?: string, funds?: readonly Coin[]): Promise<ExecuteResult> => {
    return await this.client.execute(this.sender, this.contractAddress, {
      remove_proposal_hook: {
        address
      }
    }, fee, memo, funds);
  };
  addVoteHook = async ({
    address
  }: {
    address: string;
  }, fee: number | StdFee | "auto" = "auto", memo?: string, funds?: readonly Coin[]): Promise<ExecuteResult> => {
    return await this.client.execute(this.sender, this.contractAddress, {
      add_vote_hook: {
        address
      }
    }, fee, memo, funds);
  };
  removeVoteHook = async ({
    address
  }: {
    address: string;
  }, fee: number | StdFee | "auto" = "auto", memo?: string, funds?: readonly Coin[]): Promise<ExecuteResult> => {
    return await this.client.execute(this.sender, this.contractAddress, {
      remove_vote_hook: {
        address
      }
    }, fee, memo, funds);
  };
}