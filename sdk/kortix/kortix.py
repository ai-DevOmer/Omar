from .api import agents, threads
from .agent import OMAR AIAgent
from .thread import OMAR AIThread
from .tools import AgentPressTools, MCPTools


class OMAR AI:
    def __init__(self, api_key: str, api_url="https://api.omar-ai.com/v1"):
        self._agents_client = agents.create_agents_client(api_url, api_key)
        self._threads_client = threads.create_threads_client(api_url, api_key)

        self.Agent = OMAR AIAgent(self._agents_client)
        self.Thread = OMAR AIThread(self._threads_client)
