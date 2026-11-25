// Aggregate-only lexical PGN profiler. Never emit source text or identities.
#include <algorithm>
#include <array>
#include <cctype>
#include <cstdint>
#include <fstream>
#include <iostream>
#include <map>
#include <string>
#include <string_view>
using namespace std;
using U=uint64_t;
struct G {U comments=0,vars=0,depth=0,nags=0,main=0,side=0,eval=0,clk=0,cal=0,csl=0,nonascii=0,bytes=0,maxcomment=0; bool setup=false;};
int main(int argc,char**argv){
 if(argc!=2)return 2;
 ifstream in(argv[1],ios::binary);if(!in)return 3;
 map<string,U> totals,tags; map<string,map<U,U>> hist;
 G g; bool active=false,comment=false; U depth=0,clen=0,unclosed=0; string line, token, ctext;
 auto finish_token=[&](){if(token.empty())return;auto s=string_view(token);auto p=s.find_last_of('.');if(p!=string_view::npos)s.remove_prefix(p+1); if(!s.empty()){
 char c=s.front();if(c=='$'||c=='!'||c=='?')g.nags++;else if((c>='a'&&c<='h')||string_view("KQRBNO").find(c)!=string_view::npos||s=="--"||s=="0-0"||s=="0-0-0"){(depth?g.side:g.main)++;if(s.find_first_of("!?")!=string_view::npos)g.nags++;}
 }token.clear();};
 auto finish_comment=[&](){g.comments++;g.maxcomment=max(g.maxcomment,clen);hist["comment_bytes"][clen<256?clen/16*16:clen<4096?clen/256*256:clen/4096*4096]++;g.eval+=ctext.find("[%eval")!=string::npos;g.clk+=ctext.find("[%clk")!=string::npos;g.cal+=ctext.find("[%cal")!=string::npos;g.csl+=ctext.find("[%csl")!=string::npos;ctext.clear();clen=0;};
 auto flush=[&](){if(!active)return;finish_token();if(comment||depth)unclosed++;totals["games"]++;totals["annotated_games"]+=(g.comments||g.vars||g.nags);totals["commented_games"]+=(g.comments>0);totals["variation_games"]+=(g.vars>0);totals["nag_games"]+=(g.nags>0);totals["setup_games"]+=g.setup;totals["non_ascii_comment_games"]+=(g.nonascii>0);for(auto [k,v]:initializer_list<pair<string,U>>{{"comments",g.comments},{"variations",g.vars},{"depth",g.depth},{"nags",g.nags},{"mainline_plies",g.main},{"variation_plies",g.side},{"eval_comments",g.eval},{"clock_comments",g.clk},{"arrow_comments",g.cal},{"square_comments",g.csl},{"max_comment_bytes",g.maxcomment}}){totals[k]+=v;hist[k][v]++;}g=G{};depth=0;comment=false;clen=0;ctext.clear();};
 while(getline(in,line)){
 if(!active&&line.starts_with("\xef\xbb\xbf"))line.erase(0,3);
 if(!comment&&!depth&&line.starts_with("[Event ")){flush();active=true;}
 if(!active)continue;
 if(!comment&&!depth&&line.starts_with("[")){auto p=line.find(' ');if(p!=string::npos&&p<64){auto key=line.substr(1,p-1);if(all_of(key.begin(),key.end(),[](unsigned char c){return isalnum(c)||c=='_';})){tags[key]++;if(key=="FEN")g.setup=true;continue;}}}
 for(unsigned char c:line){if(comment){if(c=='}'){finish_comment();comment=false;}else{clen++;g.nonascii+=c>=128;ctext+=char(c);}continue;}
 if(c=='{'){finish_token();comment=true;}else if(c==';'){finish_token();break;}else if(c=='('){finish_token();depth++;g.vars++;g.depth=max(g.depth,depth);}else if(c==')'){finish_token();if(depth)depth--;else totals["unmatched_close"]++;}else if(isspace(c)){finish_token();}else token+=char(c);}
 if(comment){clen++;ctext+='\n';}else finish_token();
 }
 flush();totals["unclosed_games"]=unclosed;
 cout<<"{\"totals\":{";bool first=true;for(auto&[k,v]:totals){if(!first)cout<<",";first=false;cout<<'"'<<k<<"\":"<<v;}cout<<"},\"tags\":{";first=true;for(auto&[k,v]:tags){if(!first)cout<<",";first=false;cout<<'"'<<k<<"\":"<<v;}cout<<"},\"histograms\":{";first=true;for(auto&[k,h]:hist){if(!first)cout<<",";first=false;cout<<'"'<<k<<"\":{";bool f=true;for(auto&[v,n]:h){if(!f)cout<<",";f=false;cout<<'"'<<v<<"\":"<<n;}cout<<"}";}cout<<"}}\n";
}
